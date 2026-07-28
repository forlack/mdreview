export interface ProjectInfo {
  name: string;
  root: string;
}

export interface TreeNode {
  name: string;
  path: string;
  kind: "directory" | "file";
  children?: TreeNode[];
}

export interface DocumentData {
  path: string;
  content: string;
  revision: string;
}

export type CommentStatus = "open" | "addressed" | "resolved";
export type AnchorHealth = "exact" | "moved" | "needs_review" | "orphaned";

export interface Anchor {
  revision: string;
  startByte: number;
  endByte: number;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
  renderedExact: string;
  sourceExact: string;
  prefix: string;
  suffix: string;
  health: AnchorHealth;
}

export interface ReviewComment {
  id: string;
  documentPath: string;
  status: CommentStatus;
  body: string;
  createdAt: string;
  updatedAt: string;
  originalAnchor: Anchor;
  currentAnchor: Anchor;
  resolutionNote?: string;
}

export interface ReviewTask {
  id: string;
  status: "pending" | "awaiting_review" | "complete" | "cancelled";
  commentIds: string[];
  documents: Array<{
    path: string;
    baseRevision: string;
    candidateRevision?: string;
  }>;
  createdAt: string;
  dispositions: ReviewDisposition[];
}

export interface ReviewDisposition {
  commentId: string;
  result: "addressed" | "not_addressed" | "needs_clarification";
  note: string;
}

export interface ReviewDiff {
  taskId: string;
  documents: Array<{
    path: string;
    baseRevision: string;
    candidateRevision: string;
    baseContent: string;
    candidateContent: string;
  }>;
}

export interface PendingAnchor {
  revision: string;
  startByte: number;
  endByte: number;
  renderedExact: string;
  x: number;
  y: number;
}
