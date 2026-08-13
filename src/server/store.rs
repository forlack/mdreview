use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::Utc;
use fs2::FileExt;
use ulid::Ulid;

use super::model::{
    AgentTask, Anchor, AnchorHealth, Comment, CommentStatus, DispositionResult, ReviewData,
    ReviewDisposition, ReviewTask, ReviewTaskDocument, ReviewTaskStatus,
};

const SCHEMA_GUIDE: &str = r#"# Markdown Review Data

`review.json` is managed by mdreview. Agents may read it directly when the CLI
is unavailable, but should use the CLI to report review-task results.

- `open`: feedback still needs work.
- `addressed`: an agent claims it is handled and awaits human verification.
- `resolved`: the human reviewer accepted the result.

Review tasks move from `pending` to `awaiting_review`, then to `complete` after
all addressed claims are accepted or reopened. A `cancelled` pending task no
longer reserves its comments. `review.json.backup` contains the previous valid
store state and is never used to silently overwrite a corrupt primary file.

When every request was addressed, submit without creating a report file:

`mdreview review submit <task-id> --addressed-all`

For mixed results or requests needing clarification, never silently omit a
requested comment ID. Submit a report in this format:

Report format:

```json
{
  "dispositions": [
    { "commentId": "C-...", "result": "addressed", "note": "What changed" }
  ]
}
```

Submit mixed results with
`mdreview review submit <task-id> --report <report-file>`.
"#;
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid review data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cannot load {path}: {message}. {recovery}")]
    CorruptData {
        path: String,
        message: String,
        recovery: String,
    },
    #[error("unsupported review schema version {found}; this version supports {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("review store lock is poisoned")]
    Poisoned,
    #[error("comment not found: {0}")]
    CommentNotFound(String),
    #[error("review task not found: {0}")]
    TaskNotFound(String),
    #[error("at least one open comment is required")]
    EmptyTask,
    #[error("invalid review report: {0}")]
    InvalidReport(String),
    #[error("comment belongs to an active review task: {0}")]
    CommentInTask(String),
    #[error("review task {id} is {status}; expected {expected}")]
    InvalidTaskStatus {
        id: String,
        status: String,
        expected: String,
    },
}

#[derive(Debug)]
pub struct ReviewStore {
    data_path: PathBuf,
    data: Mutex<ReviewData>,
}

impl ReviewStore {
    pub fn open(project_root: &Path) -> Result<Self, StoreError> {
        let review_directory = project_root.join(".md-review");
        let data_path = review_directory.join("review.json");
        let data = if data_path.exists() {
            read_review_data(&data_path)?
        } else {
            ReviewData::default()
        };
        Ok(Self {
            data_path,
            data: Mutex::new(data),
        })
    }

    pub fn comments(&self, path: Option<&str>) -> Result<Vec<Comment>, StoreError> {
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        Ok(data
            .comments
            .iter()
            .filter(|comment| path.is_none_or(|path| comment.document_path == path))
            .cloned()
            .collect())
    }

    pub fn initialize(&self) -> Result<(), StoreError> {
        fs::create_dir_all(self.review_directory())?;
        let schema_path = self.review_directory().join("SCHEMA.md");
        if !schema_path.exists() {
            fs::write(schema_path, SCHEMA_GUIDE)?;
        }
        Ok(())
    }

    pub fn create_comment(
        &self,
        document_path: String,
        body: String,
        anchor: Anchor,
    ) -> Result<Comment, StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let now = Utc::now().to_rfc3339();
        let comment = Comment {
            id: format!("C-{}", Ulid::new()),
            document_path,
            status: CommentStatus::Open,
            body,
            created_at: now.clone(),
            updated_at: now,
            original_anchor: anchor.clone(),
            current_anchor: anchor,
            resolution_note: None,
        };

        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        data.comments.push(comment.clone());
        self.persist(&data)?;
        Ok(comment)
    }

    pub fn update_comment(
        &self,
        id: &str,
        body: Option<String>,
        status: Option<CommentStatus>,
        resolution_note: Option<String>,
    ) -> Result<Comment, StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        if status == Some(CommentStatus::Resolved)
            && data.review_tasks.iter().any(|task| {
                task.status == ReviewTaskStatus::Pending
                    && task.comment_ids.iter().any(|comment_id| comment_id == id)
            })
        {
            return Err(StoreError::CommentInTask(id.to_owned()));
        }
        let comment = data
            .comments
            .iter_mut()
            .find(|comment| comment.id == id)
            .ok_or_else(|| StoreError::CommentNotFound(id.to_owned()))?;

        if let Some(body) = body {
            comment.body = body;
        }
        if let Some(status) = status {
            comment.status = status;
        }
        if resolution_note.is_some() {
            comment.resolution_note = resolution_note;
        }
        comment.updated_at = Utc::now().to_rfc3339();
        let updated = comment.clone();
        let addressed_ids = data
            .comments
            .iter()
            .filter(|comment| comment.status == CommentStatus::Addressed)
            .map(|comment| comment.id.clone())
            .collect::<BTreeSet<_>>();
        for task in &mut data.review_tasks {
            if task.status == ReviewTaskStatus::AwaitingReview
                && task
                    .comment_ids
                    .iter()
                    .all(|id| !addressed_ids.contains(id))
            {
                task.status = ReviewTaskStatus::Complete;
            }
        }
        self.persist(&data)?;
        Ok(updated)
    }

    pub fn delete_comment(&self, id: &str) -> Result<(), StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        if data.review_tasks.iter().any(|task| {
            matches!(
                task.status,
                ReviewTaskStatus::Pending | ReviewTaskStatus::AwaitingReview
            ) && task.comment_ids.iter().any(|comment_id| comment_id == id)
        }) {
            return Err(StoreError::CommentInTask(id.to_owned()));
        }
        let original_length = data.comments.len();
        data.comments.retain(|comment| comment.id != id);
        if data.comments.len() == original_length {
            return Err(StoreError::CommentNotFound(id.to_owned()));
        }
        self.persist(&data)?;
        Ok(())
    }

    pub fn create_task(
        &self,
        comment_ids: Vec<String>,
        documents: Vec<ReviewTaskDocument>,
    ) -> Result<ReviewTask, StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        let assigned = data
            .review_tasks
            .iter()
            .filter(|task| task.status == ReviewTaskStatus::Pending)
            .flat_map(|task| task.comment_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let open_ids = comment_ids
            .into_iter()
            .filter(|id| {
                data.comments
                    .iter()
                    .any(|comment| comment.id == *id && comment.status == CommentStatus::Open)
                    && !assigned.contains(id)
            })
            .collect::<Vec<_>>();

        if open_ids.is_empty() {
            return Err(StoreError::EmptyTask);
        }

        let task = ReviewTask {
            id: format!("task_{}", Ulid::new()),
            status: ReviewTaskStatus::Pending,
            comment_ids: open_ids,
            documents,
            created_at: Utc::now().to_rfc3339(),
            dispositions: Vec::new(),
        };
        data.review_tasks.push(task.clone());
        self.persist(&data)?;
        Ok(task)
    }

    pub fn prompt(&self, id: &str) -> Result<String, StoreError> {
        self.agent_task(id)?;
        Ok(format!(
            "Run `mdreview revise {id}` in this repository and follow the instructions it returns."
        ))
    }

    pub fn revision_instructions(&self, id: &str) -> Result<String, StoreError> {
        let agent_task = self.agent_task(id)?;
        if agent_task.task.status != ReviewTaskStatus::Pending {
            return Err(StoreError::InvalidTaskStatus {
                id: id.to_owned(),
                status: task_status_name(agent_task.task.status).into(),
                expected: "pending".into(),
            });
        }
        let mut output = format!(
            "# Revise Markdown feedback\n\n\
             Task: `{id}`\n\n\
             Follow repository instructions and use the repository for project context. Edit the \
             referenced Markdown source, preserve unrelated content, and run only checks required \
             by repository instructions for Markdown changes.\n"
        );
        let mut current_document = None;
        for comment in &agent_task.comments {
            if current_document.as_deref() != Some(comment.document_path.as_str()) {
                output.push_str(&format!("\n## `{}`\n", comment.document_path));
                current_document = Some(comment.document_path.clone());
            }
            let anchor = &comment.current_anchor;
            output.push_str(&format!(
                "\n### `{}` — line {}, column {}\n\nRequest:\n{}\n\nSelected text:\n{}\n",
                comment.id,
                anchor.start_line,
                anchor.start_column,
                markdown_quote(&comment.body),
                markdown_quote(&anchor.rendered_exact),
            ));
            if anchor.health != AnchorHealth::Exact {
                output.push_str(&format!(
                    "\nAnchor status: `{}`\n",
                    anchor_health_name(anchor.health)
                ));
            }
        }
        output.push_str(&format!(
            "\nAfter editing, if every request is addressed, submit with:\n\n\
             `mdreview review submit {id} --addressed-all`\n\n\
             If any request is not addressed or needs clarification, write a JSON report with \
             every comment ID using \
             `{{\"dispositions\":[{{\"commentId\":\"...\",\"result\":\"addressed|not_addressed|needs_clarification\",\"note\":\"...\"}}]}}` \
             and submit with `--report <report-file>` instead. Do not resolve comments; the human \
             reviewer accepts or reopens them.\n"
        ));
        Ok(output)
    }

    pub fn agent_task(&self, id: &str) -> Result<AgentTask, StoreError> {
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        let task = data
            .review_tasks
            .iter()
            .find(|task| task.id == id)
            .cloned()
            .ok_or_else(|| StoreError::TaskNotFound(id.to_owned()))?;
        let comments = task
            .comment_ids
            .iter()
            .filter_map(|id| data.comments.iter().find(|comment| comment.id == *id))
            .cloned()
            .collect();
        Ok(AgentTask { task, comments })
    }

    pub fn submit_task(
        &self,
        id: &str,
        documents: Vec<ReviewTaskDocument>,
        dispositions: Vec<ReviewDisposition>,
    ) -> Result<ReviewTask, StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        let task_index = data
            .review_tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or_else(|| StoreError::TaskNotFound(id.to_owned()))?;
        let current_status = data.review_tasks[task_index].status;
        if current_status != ReviewTaskStatus::Pending {
            return Err(StoreError::InvalidTaskStatus {
                id: id.to_owned(),
                status: task_status_name(current_status).into(),
                expected: "pending".into(),
            });
        }
        let requested = data.review_tasks[task_index]
            .comment_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let reported = dispositions
            .iter()
            .map(|item| item.comment_id.clone())
            .collect::<BTreeSet<_>>();

        if dispositions.len() != reported.len() {
            return Err(StoreError::InvalidReport(
                "a comment ID was reported more than once".into(),
            ));
        }
        if requested != reported {
            let missing = requested.difference(&reported).cloned().collect::<Vec<_>>();
            let extra = reported.difference(&requested).cloned().collect::<Vec<_>>();
            return Err(StoreError::InvalidReport(format!(
                "comment IDs must exactly match the task (missing: {}; extra: {})",
                missing.join(", "),
                extra.join(", ")
            )));
        }
        if dispositions.iter().any(|item| item.note.trim().is_empty()) {
            return Err(StoreError::InvalidReport(
                "every disposition requires a non-empty note".into(),
            ));
        }

        for disposition in &dispositions {
            if let Some(comment) = data
                .comments
                .iter_mut()
                .find(|comment| comment.id == disposition.comment_id)
            {
                comment.status = match disposition.result {
                    DispositionResult::Addressed => CommentStatus::Addressed,
                    DispositionResult::NotAddressed | DispositionResult::NeedsClarification => {
                        CommentStatus::Open
                    }
                };
                comment.updated_at = Utc::now().to_rfc3339();
            }
        }

        let has_addressed_claim = data.review_tasks[task_index].comment_ids.iter().any(|id| {
            data.comments
                .iter()
                .any(|comment| comment.id == *id && comment.status == CommentStatus::Addressed)
        });
        let task = &mut data.review_tasks[task_index];
        task.status = if has_addressed_claim {
            ReviewTaskStatus::AwaitingReview
        } else {
            ReviewTaskStatus::Complete
        };
        task.documents = documents;
        task.dispositions = dispositions;
        let submitted = task.clone();
        self.persist(&data)?;
        Ok(submitted)
    }

    pub fn cancel_task(&self, id: &str) -> Result<ReviewTask, StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        let task = data
            .review_tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or_else(|| StoreError::TaskNotFound(id.to_owned()))?;
        if task.status != ReviewTaskStatus::Pending {
            return Err(StoreError::InvalidTaskStatus {
                id: id.to_owned(),
                status: task_status_name(task.status).into(),
                expected: "pending".into(),
            });
        }
        task.status = ReviewTaskStatus::Cancelled;
        let cancelled = task.clone();
        self.persist(&data)?;
        Ok(cancelled)
    }

    pub fn review_tasks(&self) -> Result<Vec<ReviewTask>, StoreError> {
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        Ok(data.review_tasks.clone())
    }

    pub fn save_revision(&self, revision: &str, content: &str) -> Result<(), StoreError> {
        let directory = self.review_directory().join("revisions");
        fs::create_dir_all(&directory)?;
        let hash = revision.strip_prefix("sha256:").unwrap_or(revision);
        let destination = directory.join(format!("{hash}.md"));
        if !destination.exists() {
            fs::write(destination, content)?;
        }
        Ok(())
    }

    pub fn revision_content(&self, revision: &str) -> Result<String, StoreError> {
        let hash = revision.strip_prefix("sha256:").unwrap_or(revision);
        Ok(fs::read_to_string(
            self.review_directory()
                .join("revisions")
                .join(format!("{hash}.md")),
        )?)
    }

    pub fn reanchor_document(
        &self,
        path: &str,
        revision: &str,
        content: &str,
    ) -> Result<(), StoreError> {
        let _file_lock = self.exclusive_lock()?;
        let mut data = self.data.lock().map_err(|_| StoreError::Poisoned)?;
        self.refresh(&mut data)?;
        let mut changed = false;

        for comment in data.comments.iter_mut().filter(|comment| {
            comment.document_path == path && comment.current_anchor.revision != revision
        }) {
            let anchor = &comment.current_anchor;
            let matches = match_positions(content, &anchor.source_exact);
            let selected = if matches.len() == 1 {
                Some((matches[0], AnchorHealth::Moved))
            } else if matches.len() > 1 {
                best_context_match(content, &matches, anchor)
                    .map(|position| (position, AnchorHealth::Moved))
            } else {
                let rendered_matches = match_positions(content, &anchor.rendered_exact);
                if rendered_matches.len() == 1 {
                    Some((rendered_matches[0], AnchorHealth::NeedsReview))
                } else {
                    None
                }
            };

            if let Some((start, health)) = selected {
                let exact = if health == AnchorHealth::Moved {
                    &anchor.source_exact
                } else {
                    &anchor.rendered_exact
                };
                let end = start + exact.len();
                let prefix_start = previous_char_boundary(content, start.saturating_sub(160));
                let suffix_end = next_char_boundary(content, (end + 160).min(content.len()));
                let (start_line, start_column) = line_column(content, start);
                let (end_line, end_column) = line_column(content, end);
                comment.current_anchor = Anchor {
                    revision: revision.to_owned(),
                    start_byte: start,
                    end_byte: end,
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                    rendered_exact: anchor.rendered_exact.clone(),
                    source_exact: content[start..end].to_owned(),
                    prefix: content[prefix_start..start].to_owned(),
                    suffix: content[end..suffix_end].to_owned(),
                    health,
                };
            } else {
                comment.current_anchor.health = AnchorHealth::Orphaned;
            }
            comment.updated_at = Utc::now().to_rfc3339();
            changed = true;
        }

        if changed {
            self.persist(&data)?;
        }
        Ok(())
    }

    fn persist(&self, data: &ReviewData) -> Result<(), StoreError> {
        let directory = self.review_directory();
        fs::create_dir_all(directory)?;
        validate_schema(data)?;

        let schema_path = directory.join("SCHEMA.md");
        if !schema_path.exists() {
            fs::write(schema_path, SCHEMA_GUIDE)?;
        }

        if self.data_path.exists() && read_review_data(&self.data_path).is_ok() {
            fs::copy(&self.data_path, directory.join("review.json.backup"))?;
        }

        let temporary = directory.join(format!("review.json.{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(data)?;
        {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        fs::rename(temporary, &self.data_path)?;
        let backup = directory.join("review.json.backup");
        if !backup.exists() {
            fs::copy(&self.data_path, backup)?;
        }
        Ok(())
    }

    fn refresh(&self, data: &mut ReviewData) -> Result<(), StoreError> {
        if self.data_path.exists() {
            *data = read_review_data(&self.data_path)?;
        }
        Ok(())
    }

    fn review_directory(&self) -> &Path {
        self.data_path
            .parent()
            .expect("review data path always has a parent")
    }

    fn exclusive_lock(&self) -> Result<fs::File, StoreError> {
        fs::create_dir_all(self.review_directory())?;
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.review_directory().join("review.lock"))?;
        file.lock_exclusive()?;
        Ok(file)
    }
}

fn read_review_data(path: &Path) -> Result<ReviewData, StoreError> {
    let bytes = fs::read(path)?;
    let data = serde_json::from_slice::<ReviewData>(&bytes).map_err(|error| {
        let backup = path.with_file_name("review.json.backup");
        let recovery = if backup.exists() {
            format!(
                "A previous valid copy is available at {}; preserve the corrupt file, then restore the backup.",
                backup.display()
            )
        } else {
            "No backup is available; preserve the file before repairing its JSON.".into()
        };
        StoreError::CorruptData {
            path: path.display().to_string(),
            message: error.to_string(),
            recovery,
        }
    })?;
    validate_schema(&data)?;
    Ok(data)
}

fn validate_schema(data: &ReviewData) -> Result<(), StoreError> {
    if data.schema_version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema {
            found: data.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn task_status_name(status: ReviewTaskStatus) -> &'static str {
    match status {
        ReviewTaskStatus::Pending => "pending",
        ReviewTaskStatus::AwaitingReview => "awaiting_review",
        ReviewTaskStatus::Complete => "complete",
        ReviewTaskStatus::Cancelled => "cancelled",
    }
}

fn markdown_quote(value: &str) -> String {
    value
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn anchor_health_name(health: AnchorHealth) -> &'static str {
    match health {
        AnchorHealth::Exact => "exact",
        AnchorHealth::Moved => "moved",
        AnchorHealth::NeedsReview => "needs_review",
        AnchorHealth::Orphaned => "orphaned",
    }
}

fn match_positions(content: &str, exact: &str) -> Vec<usize> {
    if exact.is_empty() {
        return Vec::new();
    }
    content
        .match_indices(exact)
        .map(|(position, _)| position)
        .collect()
}

fn best_context_match(content: &str, positions: &[usize], anchor: &Anchor) -> Option<usize> {
    let mut scored = positions
        .iter()
        .map(|position| {
            let prefix = &anchor.prefix;
            let suffix = &anchor.suffix;
            let before = &content[..*position];
            let after = &content[*position + anchor.source_exact.len()..];
            let score = usize::from(before.ends_with(prefix)) * 2
                + usize::from(after.starts_with(suffix)) * 2;
            (*position, score)
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    match scored.as_slice() {
        [(position, score), ..]
            if *score > 0 && scored.get(1).is_none_or(|next| next.1 < *score) =>
        {
            Some(*position)
        }
        _ => None,
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn anchor(revision: &str) -> Anchor {
        Anchor {
            revision: revision.to_owned(),
            start_byte: 6,
            end_byte: 12,
            start_line: 1,
            start_column: 7,
            end_line: 1,
            end_column: 13,
            rendered_exact: "target".into(),
            source_exact: "target".into(),
            prefix: "Start ".into(),
            suffix: " end".into(),
            health: AnchorHealth::Exact,
        }
    }

    fn task_document() -> ReviewTaskDocument {
        ReviewTaskDocument {
            path: "doc.md".into(),
            base_revision: "sha256:old".into(),
            candidate_revision: Some("sha256:new".into()),
        }
    }

    #[test]
    fn reanchors_an_unchanged_quote_after_inserted_text() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();

        store
            .reanchor_document("doc.md", "sha256:new", "Intro\nStart target end")
            .unwrap();
        let comment = store.comments(Some("doc.md")).unwrap().remove(0);

        assert_eq!(comment.current_anchor.health, AnchorHealth::Moved);
        assert_eq!(comment.current_anchor.start_line, 2);
        assert_eq!(comment.current_anchor.source_exact, "target");
    }

    #[test]
    fn task_submission_requires_every_requested_comment() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(
                vec![comment.id],
                vec![ReviewTaskDocument {
                    path: "doc.md".into(),
                    base_revision: "sha256:old".into(),
                    candidate_revision: None,
                }],
            )
            .unwrap();

        let result = store.submit_task(&task.id, task.documents, Vec::new());
        assert!(matches!(result, Err(StoreError::InvalidReport(_))));
    }

    #[test]
    fn generated_prompt_contains_retrieval_and_submission_commands() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(
                vec![comment.id],
                vec![ReviewTaskDocument {
                    path: "doc.md".into(),
                    base_revision: "sha256:old".into(),
                    candidate_revision: None,
                }],
            )
            .unwrap();

        let prompt = store.prompt(&task.id).unwrap();
        assert!(prompt.contains("mdreview revise"));
        assert!(!prompt.contains("\"dispositions\""));
    }

    #[test]
    fn revision_instructions_include_context_comments_and_submission() {
        let directory = tempdir().unwrap();
        let store = ReviewStore::open(directory.path()).unwrap();
        let comment = store
            .create_comment("notes.md".into(), "Clarify this".into(), anchor("draft"))
            .unwrap();
        let task = store
            .create_task(
                vec![comment.id],
                vec![ReviewTaskDocument {
                    path: "notes.md".into(),
                    base_revision: "base".into(),
                    candidate_revision: None,
                }],
            )
            .unwrap();

        let instructions = store.revision_instructions(&task.id).unwrap();
        assert!(instructions.contains("Follow repository instructions"));
        assert!(instructions.contains("Clarify this"));
        assert!(instructions.contains("notes.md"));
        assert!(instructions.contains("mdreview review submit"));
        assert!(instructions.contains("--addressed-all"));
        assert!(instructions.contains("line 1, column 7"));
        assert!(instructions.contains("> target"));
        assert!(!instructions.contains("originalAnchor"));
        assert!(!instructions.contains("createdAt"));
        assert!(instructions.len() < 2_000);
    }

    #[test]
    fn cancelling_a_pending_task_releases_its_comment() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(vec![comment.id.clone()], vec![task_document()])
            .unwrap();

        let cancelled = store.cancel_task(&task.id).unwrap();
        assert_eq!(cancelled.status, ReviewTaskStatus::Cancelled);
        assert!(
            store
                .create_task(vec![comment.id], vec![task_document()])
                .is_ok()
        );
    }

    #[test]
    fn pending_tasks_cannot_be_resolved_or_submitted_twice() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(vec![comment.id.clone()], vec![task_document()])
            .unwrap();

        assert!(matches!(
            store.update_comment(&comment.id, None, Some(CommentStatus::Resolved), None),
            Err(StoreError::CommentInTask(_))
        ));
        let report = vec![ReviewDisposition {
            comment_id: comment.id,
            result: DispositionResult::Addressed,
            note: "Updated the passage".into(),
        }];
        store
            .submit_task(&task.id, vec![task_document()], report.clone())
            .unwrap();
        assert!(matches!(
            store.submit_task(&task.id, vec![task_document()], report),
            Err(StoreError::InvalidTaskStatus { .. })
        ));
    }

    #[test]
    fn non_addressed_results_finish_the_task_and_release_comments() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(vec![comment.id.clone()], vec![task_document()])
            .unwrap();
        let submitted = store
            .submit_task(
                &task.id,
                vec![task_document()],
                vec![ReviewDisposition {
                    comment_id: comment.id.clone(),
                    result: DispositionResult::NeedsClarification,
                    note: "The requested audience is unclear".into(),
                }],
            )
            .unwrap();

        assert_eq!(submitted.status, ReviewTaskStatus::Complete);
        assert_eq!(store.comments(None).unwrap()[0].status, CommentStatus::Open);
        assert!(
            store
                .create_task(vec![comment.id], vec![task_document()])
                .is_ok()
        );
    }

    #[test]
    fn reopening_the_last_addressed_comment_completes_the_task() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "Fix it".into(), anchor("sha256:old"))
            .unwrap();
        let task = store
            .create_task(vec![comment.id.clone()], vec![task_document()])
            .unwrap();
        let submitted = store
            .submit_task(
                &task.id,
                vec![task_document()],
                vec![ReviewDisposition {
                    comment_id: comment.id.clone(),
                    result: DispositionResult::Addressed,
                    note: "Updated the passage".into(),
                }],
            )
            .unwrap();
        assert_eq!(submitted.status, ReviewTaskStatus::AwaitingReview);

        store
            .update_comment(&comment.id, None, Some(CommentStatus::Open), None)
            .unwrap();
        assert_eq!(
            store.review_tasks().unwrap()[0].status,
            ReviewTaskStatus::Complete
        );
    }

    #[test]
    fn valid_review_data_is_backed_up_before_the_next_write() {
        let root = tempdir().unwrap();
        let store = ReviewStore::open(root.path()).unwrap();
        let comment = store
            .create_comment("doc.md".into(), "First".into(), anchor("sha256:old"))
            .unwrap();
        store
            .update_comment(&comment.id, Some("Second".into()), None, None)
            .unwrap();
        let backup = root.path().join(".md-review/review.json.backup");
        assert!(backup.exists());
        assert!(fs::read_to_string(&backup).unwrap().contains("First"));

        fs::write(root.path().join(".md-review/review.json"), b"{broken").unwrap();
        let error = ReviewStore::open(root.path()).unwrap_err().to_string();
        assert!(error.contains("review.json.backup"));
    }
}
