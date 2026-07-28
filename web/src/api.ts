import type {
  CommentStatus,
  DocumentData,
  PendingAnchor,
  ProjectInfo,
  ReviewComment,
  ReviewDiff,
  ReviewTask,
  TreeNode,
} from "./types";

const tokenKey = "mdreview-token";
const currentUrl = new URL(window.location.href);
const launchToken = currentUrl.searchParams.get("token");
if (launchToken) {
  sessionStorage.setItem(tokenKey, launchToken);
  currentUrl.searchParams.delete("token");
  history.replaceState(null, "", currentUrl);
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = sessionStorage.getItem(tokenKey) ?? "";
  const response = await fetch(path, {
    ...init,
    headers: {
      "content-type": "application/json",
      "x-mdreview-token": token,
      ...init?.headers,
    },
  });
  if (!response.ok) {
    const payload = (await response.json().catch(() => null)) as
      | { error?: string }
      | null;
    throw new Error(payload?.error ?? `${response.status} ${response.statusText}`);
  }
  return (await response.json()) as T;
}

export const api = {
  project: () => request<ProjectInfo>("/api/project"),
  tree: () => request<TreeNode[]>("/api/tree"),
  document: (path: string) =>
    request<DocumentData>(`/api/document?path=${encodeURIComponent(path)}`),
  assetUrl: (path: string) => {
    const token = sessionStorage.getItem(tokenKey) ?? "";
    return `/api/asset?path=${encodeURIComponent(path)}&token=${encodeURIComponent(token)}`;
  },
  comments: (path?: string) =>
    request<ReviewComment[]>(
      `/api/comments${path ? `?path=${encodeURIComponent(path)}` : ""}`,
    ),
  createComment: (documentPath: string, body: string, anchor: PendingAnchor) =>
    request<ReviewComment>("/api/comments", {
      method: "POST",
      body: JSON.stringify({
        documentPath,
        body,
        anchor: {
          revision: anchor.revision,
          startByte: anchor.startByte,
          endByte: anchor.endByte,
          renderedExact: anchor.renderedExact,
        },
      }),
    }),
  updateComment: (
    id: string,
    update: { body?: string; status?: CommentStatus; resolutionNote?: string },
  ) =>
    request<ReviewComment>(`/api/comments/${encodeURIComponent(id)}`, {
      method: "PATCH",
      body: JSON.stringify(update),
    }),
  deleteComment: async (id: string) => {
    const token = sessionStorage.getItem(tokenKey) ?? "";
    const response = await fetch(`/api/comments/${encodeURIComponent(id)}`, {
      method: "DELETE",
      headers: { "x-mdreview-token": token },
    });
    if (!response.ok) {
      const payload = (await response.json().catch(() => null)) as
        | { error?: string }
        | null;
      throw new Error(payload?.error ?? `${response.status} ${response.statusText}`);
    }
  },
  createReview: (commentIds: string[]) =>
    request<ReviewTask>("/api/reviews", {
      method: "POST",
      body: JSON.stringify({ commentIds }),
    }),
  reviewTasks: () => request<ReviewTask[]>("/api/reviews"),
  cancelReview: (id: string) =>
    request<ReviewTask>(`/api/reviews/${encodeURIComponent(id)}/cancel`, {
      method: "POST",
    }),
  reviewPrompt: async (id: string) => {
    const token = sessionStorage.getItem(tokenKey) ?? "";
    const response = await fetch(`/api/reviews/${encodeURIComponent(id)}/prompt`, {
      headers: { "x-mdreview-token": token },
    });
    if (!response.ok) throw new Error(await response.text());
    return response.text();
  },
  reviewDiff: (id: string) =>
    request<ReviewDiff>(`/api/reviews/${encodeURIComponent(id)}/diff`),
  shutdown: async () => {
    const token = sessionStorage.getItem(tokenKey) ?? "";
    const response = await fetch("/api/shutdown", {
      method: "POST",
      headers: { "x-mdreview-token": token },
    });
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  },
};
