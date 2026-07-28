use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub kind: TreeNodeKind,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TreeNodeKind {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub path: String,
    pub content: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub revision: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub rendered_exact: String,
    pub source_exact: String,
    pub prefix: String,
    pub suffix: String,
    pub health: AnchorHealth,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorHealth {
    Exact,
    Moved,
    NeedsReview,
    Orphaned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    Open,
    Addressed,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub document_path: String,
    pub status: CommentStatus,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub original_anchor: Anchor,
    pub current_anchor: Anchor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTask {
    pub id: String,
    pub status: ReviewTaskStatus,
    pub comment_ids: Vec<String>,
    pub documents: Vec<ReviewTaskDocument>,
    pub created_at: String,
    #[serde(default)]
    pub dispositions: Vec<ReviewDisposition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTaskStatus {
    Pending,
    AwaitingReview,
    Complete,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTaskDocument {
    pub path: String,
    pub base_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDisposition {
    pub comment_id: String,
    pub result: DispositionResult,
    pub note: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispositionResult {
    Addressed,
    NotAddressed,
    NeedsClarification,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTask {
    pub task: ReviewTask,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDiff {
    pub task_id: String,
    pub documents: Vec<ReviewDocumentDiff>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDocumentDiff {
    pub path: String,
    pub base_revision: String,
    pub candidate_revision: String,
    pub base_content: String,
    pub candidate_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewData {
    pub schema_version: u32,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub review_tasks: Vec<ReviewTask>,
}

impl Default for ReviewData {
    fn default() -> Self {
        Self {
            schema_version: 1,
            comments: Vec::new(),
            review_tasks: Vec::new(),
        }
    }
}
