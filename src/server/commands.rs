use std::{fs, path::PathBuf};

use serde::Deserialize;

use super::{
    model::{
        CommentStatus, DispositionResult, ReviewDisposition, ReviewTaskDocument, ReviewTaskStatus,
    },
    project::Project,
    store::ReviewStore,
};

const AGENTS_BLOCK: &str = r#"<!-- md-review:managed:start -->
## Markdown review workflow

When asked to address Markdown review comments, run
`mdreview revise <task-id>` using the task ID in the handoff prompt. This
returns the complete task instructions and anchored comments. If the command is
unavailable, read `.md-review/review.json` and `.md-review/SCHEMA.md`.

Edit the referenced Markdown source, then follow the submission command returned
by `mdreview revise`. Never mark comments resolved; resolution belongs to the
human reviewer.
<!-- md-review:managed:end -->"#;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Project(#[from] super::project::ProjectError),
    #[error("{0}")]
    Store(#[from] super::store::StoreError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid report JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn list_comments(
    project_path: PathBuf,
    open_only: bool,
    document: Option<String>,
) -> Result<(), CommandError> {
    let project = Project::open(project_path)?;
    let store = ReviewStore::open(project.root())?;
    let mut comments = store.comments(document.as_deref())?;
    if open_only {
        comments.retain(|comment| comment.status == CommentStatus::Open);
    }
    println!("{}", serde_json::to_string_pretty(&comments)?);
    Ok(())
}

pub fn initialize(project_path: PathBuf, append: bool) -> Result<(), CommandError> {
    let project = Project::open(project_path)?;
    let store = ReviewStore::open(project.root())?;
    store.initialize()?;

    let agents_path = project.root().join("AGENTS.md");
    if !agents_path.exists() {
        fs::write(&agents_path, format!("{AGENTS_BLOCK}\n"))?;
        println!("Created {}", agents_path.display());
        return Ok(());
    }

    let existing = fs::read_to_string(&agents_path)?;
    let start_marker = "<!-- md-review:managed:start -->";
    let end_marker = "<!-- md-review:managed:end -->";
    if let (Some(start), Some(end_start)) = (existing.find(start_marker), existing.find(end_marker))
    {
        let end = end_start + end_marker.len();
        let mut updated = String::new();
        updated.push_str(&existing[..start]);
        updated.push_str(AGENTS_BLOCK);
        updated.push_str(&existing[end..]);
        fs::write(&agents_path, updated)?;
        println!(
            "Updated the managed review block in {}",
            agents_path.display()
        );
    } else if append {
        let separator = if existing.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        fs::write(
            &agents_path,
            format!("{existing}{separator}{AGENTS_BLOCK}\n"),
        )?;
        println!("Appended the review block to {}", agents_path.display());
    } else {
        println!(
            "{} already exists. Re-run `mdreview init --append` to add this managed block:\n\n{}",
            agents_path.display(),
            AGENTS_BLOCK
        );
    }
    Ok(())
}

pub fn show_task(project_path: PathBuf, id: &str) -> Result<(), CommandError> {
    let project = Project::open(project_path)?;
    let store = ReviewStore::open(project.root())?;
    println!("{}", serde_json::to_string_pretty(&store.agent_task(id)?)?);
    Ok(())
}

pub fn revise_task(project_path: PathBuf, id: &str) -> Result<(), CommandError> {
    let project = Project::open(project_path)?;
    let store = ReviewStore::open(project.root())?;
    println!("{}", store.revision_instructions(id)?);
    Ok(())
}

pub fn submit_task(
    project_path: PathBuf,
    id: &str,
    report_path: Option<PathBuf>,
    addressed_all: bool,
) -> Result<(), CommandError> {
    let project = Project::open(project_path)?;
    let store = ReviewStore::open(project.root())?;
    let task = store.agent_task(id)?.task;
    let dispositions = if addressed_all {
        task.comment_ids
            .iter()
            .map(|comment_id| ReviewDisposition {
                comment_id: comment_id.clone(),
                result: DispositionResult::Addressed,
                note: "Addressed in the candidate revision.".into(),
            })
            .collect()
    } else {
        let report_path = report_path.expect("clap requires --report or --addressed-all");
        let report: AgentReport = serde_json::from_slice(&fs::read(report_path)?)?;
        report.dispositions
    };
    let documents = task
        .documents
        .iter()
        .map(|document| {
            let current = project.document(&document.path)?;
            store.save_revision(&current.revision, &current.content)?;
            Ok(ReviewTaskDocument {
                path: document.path.clone(),
                base_revision: document.base_revision.clone(),
                candidate_revision: Some(current.revision),
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let submitted = store.submit_task(id, documents, dispositions)?;
    let status = match submitted.status {
        ReviewTaskStatus::Pending => "pending",
        ReviewTaskStatus::AwaitingReview => "awaiting review",
        ReviewTaskStatus::Complete => "complete",
        ReviewTaskStatus::Cancelled => "cancelled",
    };
    let result_label = if submitted.dispositions.len() == 1 {
        "result"
    } else {
        "results"
    };
    println!(
        "Submitted {id}: {} comment {result_label} recorded; task is {status}.",
        submitted.dispositions.len(),
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentReport {
    dispositions: Vec<ReviewDisposition>,
}
