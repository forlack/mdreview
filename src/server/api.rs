use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::sync::watch;

use super::{
    model::{
        Anchor, AnchorHealth, CommentStatus, ReviewDiff, ReviewDocumentDiff, ReviewTaskDocument,
    },
    project::{Project, ProjectError},
    store::{ReviewStore, StoreError},
};

#[derive(Clone)]
pub struct AppState {
    pub project: Arc<Project>,
    pub store: Arc<ReviewStore>,
    pub token: String,
    pub shutdown: watch::Sender<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Project(#[from] ProjectError),
    #[error("{0}")]
    Store(#[from] StoreError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Project(ProjectError::InvalidPath | ProjectError::NotMarkdown(_)) => {
                StatusCode::BAD_REQUEST
            }
            Self::Project(ProjectError::OutsideProject) => StatusCode::FORBIDDEN,
            Self::Project(_) => StatusCode::NOT_FOUND,
            Self::Store(StoreError::CommentNotFound(_) | StoreError::TaskNotFound(_)) => {
                StatusCode::NOT_FOUND
            }
            Self::Store(_) => StatusCode::BAD_REQUEST,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

pub async fn get_project(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.project.info()))
}

pub async fn shutdown_server(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    let _ = state.shutdown.send(true);
    Ok(StatusCode::ACCEPTED)
}

pub async fn get_tree(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.project.tree()?))
}

pub async fn get_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DocumentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    let document = state.project.document(&query.path)?;
    state
        .store
        .reanchor_document(&document.path, &document.revision, &document.content)?;
    Ok(Json(document))
}

pub async fn get_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AssetQuery>,
) -> Result<Response, ApiError> {
    authorize_with_query(&state, &headers, query.token.as_deref())?;
    let bytes = state.project.asset(&query.path)?;
    let mime = mime_guess::from_path(&query.path).first_or_octet_stream();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .expect("asset response headers are valid"))
}

pub async fn get_comments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CommentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.comments(query.path.as_deref())?))
}

pub async fn create_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateCommentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    let body = request.body.trim();
    if body.is_empty() {
        return Err(ApiError::BadRequest("comment cannot be empty".into()));
    }

    let document = state.project.document(&request.document_path)?;
    if document.revision != request.anchor.revision {
        return Err(ApiError::BadRequest(
            "document changed after the selection; reload and try again".into(),
        ));
    }
    if request.anchor.start_byte >= request.anchor.end_byte
        || request.anchor.end_byte > document.content.len()
        || !document.content.is_char_boundary(request.anchor.start_byte)
        || !document.content.is_char_boundary(request.anchor.end_byte)
    {
        return Err(ApiError::BadRequest("invalid source selection".into()));
    }

    let source_exact =
        document.content[request.anchor.start_byte..request.anchor.end_byte].to_owned();
    let prefix_start = previous_char_boundary(
        &document.content,
        request.anchor.start_byte.saturating_sub(160),
    );
    let suffix_end = next_char_boundary(
        &document.content,
        (request.anchor.end_byte + 160).min(document.content.len()),
    );
    let (start_line, start_column) = line_column(&document.content, request.anchor.start_byte);
    let (end_line, end_column) = line_column(&document.content, request.anchor.end_byte);
    let anchor = Anchor {
        revision: document.revision,
        start_byte: request.anchor.start_byte,
        end_byte: request.anchor.end_byte,
        start_line,
        start_column,
        end_line,
        end_column,
        rendered_exact: request.anchor.rendered_exact,
        source_exact,
        prefix: document.content[prefix_start..request.anchor.start_byte].to_owned(),
        suffix: document.content[request.anchor.end_byte..suffix_end].to_owned(),
        health: AnchorHealth::Exact,
    };

    let comment = state
        .store
        .create_comment(request.document_path, body.to_owned(), anchor)?;
    Ok((StatusCode::CREATED, Json(comment)))
}

pub async fn update_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCommentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    if request
        .body
        .as_ref()
        .is_some_and(|body| body.trim().is_empty())
    {
        return Err(ApiError::BadRequest("comment cannot be empty".into()));
    }
    Ok(Json(state.store.update_comment(
        &id,
        request.body.map(|body| body.trim().to_owned()),
        request.status,
        request.resolution_note,
    )?))
}

pub async fn delete_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    state.store.delete_comment(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_review_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateReviewTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    let comments = state.store.comments(None)?;
    let mut documents = BTreeMap::new();
    for id in &request.comment_ids {
        if let Some(comment) = comments.iter().find(|comment| comment.id == *id) {
            let document = state.project.document(&comment.document_path)?;
            state
                .store
                .save_revision(&document.revision, &document.content)?;
            documents.insert(
                comment.document_path.clone(),
                ReviewTaskDocument {
                    path: comment.document_path.clone(),
                    base_revision: document.revision,
                    candidate_revision: None,
                },
            );
        }
    }
    let task = state
        .store
        .create_task(request.comment_ids, documents.into_values().collect())?;
    Ok((StatusCode::CREATED, Json(task)))
}

pub async fn get_review_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.review_tasks()?))
}

pub async fn get_review_prompt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok((
        [("content-type", "text/plain; charset=utf-8")],
        state.store.prompt(&id)?,
    ))
}

pub async fn cancel_review_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    Ok(Json(state.store.cancel_task(&id)?))
}

pub async fn get_review_diff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &headers)?;
    let task = state.store.agent_task(&id)?.task;
    let documents = task
        .documents
        .iter()
        .filter_map(|document| {
            document
                .candidate_revision
                .as_ref()
                .map(|candidate| (document, candidate))
        })
        .map(|(document, candidate)| {
            Ok(ReviewDocumentDiff {
                path: document.path.clone(),
                base_revision: document.base_revision.clone(),
                candidate_revision: candidate.clone(),
                base_content: state.store.revision_content(&document.base_revision)?,
                candidate_content: state.store.revision_content(candidate)?,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(Json(ReviewDiff {
        task_id: task.id,
        documents,
    }))
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let supplied = headers
        .get("x-mdreview-token")
        .and_then(|value| value.to_str().ok());
    if supplied == Some(state.token.as_str()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

fn authorize_with_query(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), ApiError> {
    if query_token == Some(state.token.as_str()) {
        Ok(())
    } else {
        authorize(state, headers)
    }
}

fn line_column(content: &str, byte: usize) -> (usize, usize) {
    let before = &content[..byte];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, current)| current)
        .chars()
        .count()
        + 1;
    (line, column)
}

fn previous_char_boundary(content: &str, mut byte: usize) -> usize {
    while byte > 0 && !content.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn next_char_boundary(content: &str, mut byte: usize) -> usize {
    while byte < content.len() && !content.is_char_boundary(byte) {
        byte += 1;
    }
    byte
}

#[derive(Debug, Deserialize)]
pub struct DocumentQuery {
    path: String,
}

#[derive(Debug, Deserialize)]
pub struct AssetQuery {
    path: String,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommentQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    document_path: String,
    body: String,
    anchor: CreateAnchorRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnchorRequest {
    revision: String,
    start_byte: usize,
    end_byte: usize,
    rendered_exact: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommentRequest {
    body: Option<String>,
    status: Option<CommentStatus>,
    resolution_note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReviewTaskRequest {
    comment_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_columns_are_one_based_and_unicode_aware() {
        let content = "one\n🙂two";
        let byte = content.find("two").unwrap();
        assert_eq!(line_column(content, byte), (2, 2));
    }
}
