mod api;
mod assets;
pub mod commands;
pub(crate) mod model;
pub(crate) mod project;
pub(crate) mod store;

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    routing::{get, patch, post},
};
use rand::RngCore;
use tokio::{net::TcpListener, sync::watch};

use self::{api::AppState, project::Project, store::ReviewStore};

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("cannot open project: {0}")]
    Project(#[from] project::ProjectError),
    #[error("cannot open review store: {0}")]
    Store(#[from] store::StoreError),
    #[error("cannot bind local server: {0}")]
    Bind(#[from] std::io::Error),
}

pub async fn run(path: PathBuf, no_open: bool) -> Result<(), ServerError> {
    let project = Arc::new(Project::open(path)?);
    let project_path = project.root().display().to_string();
    let store = Arc::new(ReviewStore::open(project.root())?);
    let token = random_token();
    let (shutdown, shutdown_receiver) = watch::channel(false);
    let state = AppState {
        project,
        store,
        token: token.clone(),
        shutdown,
    };

    let app = Router::new()
        .route("/api/project", get(api::get_project))
        .route("/api/tree", get(api::get_tree))
        .route("/api/document", get(api::get_document))
        .route("/api/asset", get(api::get_asset))
        .route(
            "/api/comments",
            get(api::get_comments).post(api::create_comment),
        )
        .route(
            "/api/comments/{id}",
            patch(api::update_comment).delete(api::delete_comment),
        )
        .route(
            "/api/reviews",
            get(api::get_review_tasks).post(api::create_review_task),
        )
        .route("/api/reviews/{id}/prompt", get(api::get_review_prompt))
        .route("/api/reviews/{id}/diff", get(api::get_review_diff))
        .route("/api/reviews/{id}/cancel", post(api::cancel_review_task))
        .route("/api/shutdown", post(api::shutdown_server))
        .fallback(get(assets::serve))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/?token={token}");

    println!("Reviewing {project_path}");
    println!("Open {url}");

    if !no_open {
        let _ = webbrowser::open(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_receiver))
        .await?;
    Ok(())
}

fn random_token() -> String {
    let mut bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

async fn shutdown_signal(mut requested: watch::Receiver<bool>) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = requested.wait_for(|shutdown| *shutdown) => {}
    }
}
