import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";

import { api } from "./api";
import { MarkdownView } from "./MarkdownView";
import { captureSelection } from "./selection";
import { UnifiedDiff } from "./UnifiedDiff";
import type {
  DocumentData,
  PendingAnchor,
  ProjectInfo,
  ReviewComment,
  ReviewDiff,
  ReviewDisposition,
  ReviewTask,
  TreeNode,
} from "./types";

type ReadingDensity = "comfortable" | "compact";
type ThemeId = "paper" | "daylight" | "forest" | "midnight" | "charcoal";

const READING_DENSITY_KEY = "mdreview-reading-density";
const THEME_KEY = "mdreview-theme";
const THEMES: Array<{ id: ThemeId; label: string }> = [
  { id: "paper", label: "Paper · Light" },
  { id: "daylight", label: "Daylight · Light" },
  { id: "forest", label: "Forest · Dark" },
  { id: "midnight", label: "Midnight · Dark" },
  { id: "charcoal", label: "Charcoal · Dark" },
];

export function App() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [document, setDocument] = useState<DocumentData | null>(null);
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [projectComments, setProjectComments] = useState<ReviewComment[]>([]);
  const [tasks, setTasks] = useState<ReviewTask[]>([]);
  const [pending, setPending] = useState<PendingAnchor | null>(null);
  const [composing, setComposing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showResolved, setShowResolved] = useState(false);
  const [prompt, setPrompt] = useState<string | null>(null);
  const [reviewDiff, setReviewDiff] = useState<ReviewDiff | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [copiedPrompt, setCopiedPrompt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [shuttingDown, setShuttingDown] = useState(false);
  const [mobilePanel, setMobilePanel] = useState<"files" | "comments" | null>(null);
  const [theme, setTheme] = useState<ThemeId>(initialTheme);
  const [readingDensity, setReadingDensity] = useState<ReadingDensity>(() => {
    return storedPreference(READING_DENSITY_KEY) === "comfortable"
      ? "comfortable"
      : "compact";
  });
  const article = useRef<HTMLElement | null>(null);

  useEffect(() => {
    window.document.documentElement.dataset.theme = theme;
    rememberPreference(THEME_KEY, theme);
  }, [theme]);

  useEffect(() => {
    Promise.all([api.project(), api.tree()])
      .then(([projectInfo, nodes]) => {
        setProject(projectInfo);
        setTree(nodes);
        setSelectedPath(firstFile(nodes));
      })
      .catch(showError);
  }, []);

  useEffect(() => {
    if (!selectedPath || shuttingDown) return;
    let cancelled = false;
    setDocument(null);
    setPending(null);
    const load = async () => {
      try {
        const nextTree = await api.tree();
        if (cancelled) return;
        setTree(nextTree);
        if (!treeContains(nextTree, selectedPath)) {
          setSelectedPath(firstFile(nextTree));
          return;
        }
        const nextDocument = await api.document(selectedPath);
        const [nextComments, nextProjectComments, nextTasks] = await Promise.all([
          api.comments(selectedPath),
          api.comments(),
          api.reviewTasks(),
        ]);
        if (!cancelled) {
          setDocument(nextDocument);
          setComments(nextComments);
          setProjectComments(nextProjectComments);
          setTasks(nextTasks);
        }
      } catch (problem) {
        if (!cancelled) showError(problem);
      }
    };
    void load();
    const poll = window.setInterval(load, 3000);
    return () => {
      cancelled = true;
      window.clearInterval(poll);
    };
  }, [selectedPath, shuttingDown]);

  useEffect(() => {
    if (!message) return;
    const timeout = window.setTimeout(() => setMessage(null), 3000);
    return () => window.clearTimeout(timeout);
  }, [message]);

  useEffect(() => {
    if (!copiedPrompt) return;
    const timeout = window.setTimeout(() => setCopiedPrompt(null), 3000);
    return () => window.clearTimeout(timeout);
  }, [copiedPrompt]);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (reviewDiff) setReviewDiff(null);
      else if (prompt) setPrompt(null);
      else if (composing) setComposing(false);
      else if (mobilePanel) setMobilePanel(null);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [composing, mobilePanel, prompt, reviewDiff]);

  useEffect(() => {
    if (!composing && !prompt && !reviewDiff) return;
    const dialog = window.document.querySelector<HTMLElement>("[role='dialog']");
    if (!dialog) return;
    const previous = window.document.activeElement as HTMLElement | null;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        "button:not(:disabled), textarea:not(:disabled), select:not(:disabled), [href], [tabindex]:not([tabindex='-1'])",
      ),
    );
    focusable[0]?.focus();
    const trapFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && window.document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && window.document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    dialog.addEventListener("keydown", trapFocus);
    return () => {
      dialog.removeEventListener("keydown", trapFocus);
      previous?.focus();
    };
  }, [composing, prompt !== null, reviewDiff !== null]);

  const activeComments = useMemo(
    () => comments.filter((comment) => comment.status !== "resolved"),
    [comments],
  );
  const visibleComments = useMemo(
    () => (showResolved ? comments : comments.filter((comment) => comment.status !== "resolved")),
    [comments, showResolved],
  );
  const sendableComments = useMemo(() => {
    const assigned = new Set(
      tasks
        .filter((task) => task.status === "pending")
        .flatMap((task) => task.commentIds),
    );
    return projectComments.filter(
      (comment) => comment.status === "open" && !assigned.has(comment.id),
    );
  }, [projectComments, tasks]);
  const taskHistory = useMemo(() => [...tasks].reverse(), [tasks]);

  function showError(problem: unknown) {
    setError(problem instanceof Error ? problem.message : String(problem));
  }

  function handleSelection() {
    if (!article.current || !document || composing) return;
    const next = captureSelection(article.current, document.content, document.revision);
    setPending(next);
  }

  async function submitComment(body: string) {
    if (!pending || !document || !body.trim()) return;
    try {
      const created = await api.createComment(document.path, body, pending);
      setComments((current) => [...current, created]);
      setProjectComments((current) => [...current, created]);
      setComposing(false);
      setPending(null);
      window.getSelection()?.removeAllRanges();
      setMessage("Comment saved");
    } catch (problem) {
      showError(problem);
    }
  }

  async function setStatus(comment: ReviewComment, status: "open" | "resolved") {
    try {
      const updated = await api.updateComment(comment.id, { status });
      setComments((current) =>
        current.map((candidate) => (candidate.id === updated.id ? updated : candidate)),
      );
      setProjectComments((current) =>
        current.map((candidate) => (candidate.id === updated.id ? updated : candidate)),
      );
    } catch (problem) {
      showError(problem);
    }
  }

  async function saveEditedComment(comment: ReviewComment, body: string) {
    if (!body.trim()) return;
    try {
      const updated = await api.updateComment(comment.id, { body });
      setComments((current) =>
        current.map((candidate) => (candidate.id === updated.id ? updated : candidate)),
      );
      setProjectComments((current) =>
        current.map((candidate) => (candidate.id === updated.id ? updated : candidate)),
      );
      setEditingId(null);
      setMessage("Comment updated");
    } catch (problem) {
      showError(problem);
    }
  }

  async function deleteComment(comment: ReviewComment) {
    if (!window.confirm("Delete this review comment?")) return;
    try {
      await api.deleteComment(comment.id);
      setComments((current) => current.filter((candidate) => candidate.id !== comment.id));
      setProjectComments((current) =>
        current.filter((candidate) => candidate.id !== comment.id),
      );
      setMessage("Comment deleted");
    } catch (problem) {
      showError(problem);
    }
  }

  async function copyAgentPrompt(task: ReviewTask) {
    const nextPrompt = await api.reviewPrompt(task.id);
    try {
      await navigator.clipboard.writeText(nextPrompt);
      setCopiedPrompt(nextPrompt);
    } catch {
      setPrompt(nextPrompt);
      setMessage("Clipboard access failed; copy the prompt manually");
    }
  }

  async function sendToAgent() {
    try {
      const task = await api.createReview(sendableComments.map((comment) => comment.id));
      setTasks((current) => [...current, task]);
      await copyAgentPrompt(task);
    } catch (problem) {
      showError(problem);
    }
  }

  async function cancelTask(task: ReviewTask) {
    if (!window.confirm(`Cancel ${task.id}? Its comments will become available to send again.`)) {
      return;
    }
    try {
      const cancelled = await api.cancelReview(task.id);
      setTasks((current) =>
        current.map((candidate) => (candidate.id === cancelled.id ? cancelled : candidate)),
      );
      setMessage("Agent task cancelled");
    } catch (problem) {
      showError(problem);
    }
  }

  async function showChanges(task: ReviewTask) {
    try {
      setReviewDiff(await api.reviewDiff(task.id));
    } catch (problem) {
      showError(problem);
    }
  }

  function latestDisposition(commentId: string): ReviewDisposition | undefined {
    return [...tasks]
      .reverse()
      .flatMap((task) => task.dispositions)
      .find((disposition) => disposition.commentId === commentId);
  }

  async function copyPrompt() {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt);
      setCopiedPrompt(prompt);
      setPrompt(null);
    } catch {
      setMessage("Clipboard access failed; select and copy the prompt manually");
    }
  }

  function focusComment(comment: ReviewComment) {
    const exactMatch = article.current?.querySelector<HTMLElement>(
      `[data-comment-ids~="${CSS.escape(comment.id)}"]`,
    );
    const match =
      exactMatch ?? closestRenderedLocation(article.current, comment.currentAnchor.startByte);
    if (!match) return;

    match?.scrollIntoView({ behavior: "smooth", block: "center" });
    const color = exactMatch ? "var(--comment-focus)" : "var(--comment-focus-approximate)";
    match.animate(
      [
        { backgroundColor: color, boxShadow: `0 0 0 3px ${color}` },
        { backgroundColor: color, boxShadow: `0 0 0 3px ${color}`, offset: 0.68 },
        { backgroundColor: "transparent", boxShadow: "0 0 0 3px transparent" },
      ],
      { duration: 1800, easing: "ease-out" },
    );
  }

  function changeReadingDensity(next: ReadingDensity) {
    setReadingDensity(next);
    rememberPreference(READING_DENSITY_KEY, next);
  }

  async function shutdown() {
    if (!window.confirm("Shut down mdreview? The browser tab will remain open.")) return;
    setShuttingDown(true);
    try {
      await api.shutdown();
    } catch (problem) {
      setShuttingDown(false);
      showError(problem);
    }
  }

  const setArticleRef = useCallback((element: HTMLElement | null) => {
    article.current = element;
  }, []);

  const navigateToDocument = useCallback((path: string) => {
    setSelectedPath(path);
    setMobilePanel(null);
  }, []);

  if (shuttingDown) {
    return (
      <main class="stopped-screen">
        <div>
          <h1>mdreview stopped</h1>
          <p>The local review process has exited. You can close this tab.</p>
        </div>
      </main>
    );
  }

  return (
    <div class="app-shell">
      <header class="topbar">
        <div class="topbar-title">
          <strong>{project?.name ?? "Markdown Review"}</strong>
          {project && <span class="project-root">{project.root}</span>}
        </div>
        <div class="topbar-actions">
          {message && <span class="message">{message}</span>}
          <button
            class="mobile-nav-button"
            aria-label="Open Markdown files"
            aria-expanded={mobilePanel === "files"}
            onClick={() => setMobilePanel((current) => current === "files" ? null : "files")}
          >
            <span aria-hidden="true">☰</span><span class="mobile-button-label">Files</span>
          </button>
          <button
            class="mobile-nav-button"
            aria-label="Open review comments"
            aria-expanded={mobilePanel === "comments"}
            onClick={() => setMobilePanel((current) => current === "comments" ? null : "comments")}
          >
            <span aria-hidden="true">◫</span><span class="mobile-button-label">Comments</span>
          </button>
          <label class="theme-setting">
            <span>Theme</span>
            <select
              aria-label="Color theme"
              value={theme}
              onChange={(event) => setTheme(event.currentTarget.value as ThemeId)}
            >
              {THEMES.map((option) => (
                <option value={option.id} key={option.id}>{option.label}</option>
              ))}
            </select>
          </label>
          <label class="density-setting">
            <span>Density</span>
            <select
              aria-label="Markdown reading density"
              value={readingDensity}
              onChange={(event) =>
                changeReadingDensity(event.currentTarget.value as ReadingDensity)
              }
            >
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
            </select>
          </label>
          <button
            class="primary"
            aria-label={`Send ${sendableComments.length} comments to agent`}
            disabled={sendableComments.length === 0}
            onClick={sendToAgent}
          >
            <span class="send-label">Send to agent</span> ({sendableComments.length})
          </button>
          {tasks
            .filter((task) => task.status === "awaiting_review")
            .slice(-1)
            .map((task) => (
              <button class="secondary" key={task.id} onClick={() => showChanges(task)}>
                Review changes
              </button>
            ))}
          <button class="shutdown-button" aria-label="Shut down mdreview" onClick={shutdown}>
            Shutdown
          </button>
        </div>
      </header>

      <aside class={`file-panel ${mobilePanel === "files" ? "mobile-open" : ""}`} aria-label="Markdown files">
        <div class="panel-heading">Files</div>
        {tree.length ? (
          <Tree
            nodes={tree}
            selected={selectedPath}
            onSelect={(path) => {
              setSelectedPath(path);
              setMobilePanel(null);
            }}
          />
        ) : (
          <p class="empty">No Markdown files found.</p>
        )}
      </aside>

      <main class={`document-panel density-${readingDensity}`}>
        {document ? (
          <MarkdownView
            source={document.content}
            revision={document.revision}
            comments={comments}
            documentPath={document.path}
            articleRef={setArticleRef}
            onSelection={handleSelection}
            onNavigate={navigateToDocument}
          />
        ) : selectedPath ? (
          <div class="loading">Loading document…</div>
        ) : (
          <div class="empty-state">Choose a Markdown file to begin reviewing.</div>
        )}
      </main>

      <aside class={`comment-panel ${mobilePanel === "comments" ? "mobile-open" : ""}`} aria-label="Review comments">
        <div class="panel-heading">
          Comments <span class="count">{activeComments.length}</span>
        </div>
        {taskHistory.length > 0 && (
          <section class="review-tasks" aria-label="Review task history">
            <div class="review-tasks-heading">Review tasks</div>
            {taskHistory.map((task) => (
              <div class={`review-task status-${task.status}`} key={task.id}>
                <div class="review-task-summary">
                  <code title={task.id}>{task.id}</code>
                  <span>{task.status.replaceAll("_", " ")}</span>
                </div>
                <div class="review-task-actions">
                  {task.status === "pending" && (
                    <>
                      <button onClick={() => copyAgentPrompt(task).catch(showError)}>
                        Copy prompt
                      </button>
                      <button onClick={() => cancelTask(task)}>Cancel</button>
                    </>
                  )}
                  {task.documents.some((item) => item.candidateRevision) && (
                    <button onClick={() => showChanges(task)}>View changes</button>
                  )}
                </div>
              </div>
            ))}
          </section>
        )}
        <div class="comment-list">
          {comments.some((comment) => comment.status === "resolved") && (
            <button class="resolved-toggle" onClick={() => setShowResolved((value) => !value)}>
              {showResolved ? "Hide resolved" : "Show resolved"}
            </button>
          )}
          {visibleComments.length === 0 && (
            <p class="empty">
              {comments.length ? "No active comments." : "Select text to leave a comment."}
            </p>
          )}
          {visibleComments.map((comment) => (
            <div class={`comment-card status-${comment.status}`} key={comment.id}>
              <button class="comment-target" onClick={() => focusComment(comment)}>
                <span>{comment.currentAnchor.startLine}:{comment.currentAnchor.startColumn}</span>
                <q>{comment.currentAnchor.renderedExact}</q>
              </button>
              {editingId === comment.id ? (
                <CommentEditor
                  comment={comment}
                  onCancel={() => setEditingId(null)}
                  onSave={(body) => saveEditedComment(comment, body)}
                />
              ) : (
                <p>{comment.body}</p>
              )}
              {latestDisposition(comment.id) && (
                <div class="agent-response">
                  <strong>Agent response</strong>
                  <p>{latestDisposition(comment.id)?.note}</p>
                </div>
              )}
              <div class="comment-meta">
                <span>
                  {comment.status.replace("_", " ")}
                  {comment.currentAnchor.health !== "exact" &&
                    ` · ${comment.currentAnchor.health.replace("_", " ")}`}
                </span>
                {comment.status === "resolved" ? (
                  <button onClick={() => setStatus(comment, "open")}>Reopen</button>
                ) : comment.status === "addressed" ? (
                  <span class="review-actions">
                    <button onClick={() => setStatus(comment, "open")}>Reopen</button>
                    <button onClick={() => setStatus(comment, "resolved")}>Accept</button>
                  </span>
                ) : (
                  <button onClick={() => setStatus(comment, "resolved")}>Resolve</button>
                )}
              </div>
              <div class="comment-tools">
                <button
                  onClick={() => {
                    setEditingId(comment.id);
                  }}
                >
                  Edit
                </button>
                <button onClick={() => deleteComment(comment)}>Delete</button>
              </div>
            </div>
          ))}
        </div>
      </aside>

      {mobilePanel && (
        <button
          class="drawer-backdrop"
          aria-label="Close navigation panel"
          onClick={() => setMobilePanel(null)}
        />
      )}

      {pending && !composing && (
        <button
          class="selection-action"
          style={{ left: pending.x, top: pending.y }}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setComposing(true)}
        >
          Comment
        </button>
      )}

      {pending && composing && (
        <CommentComposer
          quote={pending.renderedExact}
          onCancel={() => setComposing(false)}
          onSubmit={submitComment}
        />
      )}

      {prompt && (
        <div class="composer-backdrop" onMouseDown={() => setPrompt(null)}>
          <section
            class="prompt-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="agent-prompt-title"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <h2 id="agent-prompt-title">Agent prompt</h2>
            <p>Paste this into an agent running in the project folder.</p>
            <textarea
              rows={13}
              value={prompt}
              onInput={(event) => setPrompt(event.currentTarget.value)}
            />
            <div class="dialog-actions">
              <button onClick={() => setPrompt(null)}>Cancel</button>
              <button class="primary" onClick={copyPrompt}>Copy prompt</button>
            </div>
          </section>
        </div>
      )}

      {reviewDiff && (
        <div class="composer-backdrop" onMouseDown={() => setReviewDiff(null)}>
          <section
            class="diff-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="candidate-changes-title"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div class="diff-heading">
              <div>
                <h2 id="candidate-changes-title">Candidate changes</h2>
                <span>{reviewDiff.taskId}</span>
              </div>
              <button onClick={() => setReviewDiff(null)}>Close</button>
            </div>
            {reviewDiff.documents.map((item) => (
              <section class="document-diff" key={item.path}>
                <h3>{item.path}</h3>
                <UnifiedDiff before={item.baseContent} after={item.candidateContent} />
              </section>
            ))}
          </section>
        </div>
      )}

      {error && (
        <div class="error-toast" role="alert">
          <span>{error}</span>
          <button aria-label="Dismiss error" onClick={() => setError(null)}>×</button>
        </div>
      )}

      {copiedPrompt && (
        <div class="clipboard-toast" role="status">
          <span>Prompt</span>
          <q>{copiedPrompt}</q>
          <span>sent to clipboard.</span>
        </div>
      )}
    </div>
  );
}

function CommentComposer({
  quote,
  onCancel,
  onSubmit,
}: {
  quote: string;
  onCancel: () => void;
  onSubmit: (body: string) => Promise<void>;
}) {
  const [body, setBody] = useState("");

  return (
    <div class="composer-backdrop" onMouseDown={onCancel}>
      <form
        class="composer"
        role="dialog"
        aria-modal="true"
        aria-label="Add review comment"
        onSubmit={(event) => {
          event.preventDefault();
          void onSubmit(body);
        }}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <q>{quote}</q>
        <textarea
          autofocus
          rows={4}
          placeholder="What should change?"
          value={body}
          onInput={(event) => setBody(event.currentTarget.value)}
        />
        <div class="dialog-actions">
          <button type="button" onClick={onCancel}>Cancel</button>
          <button class="primary" disabled={!body.trim()} type="submit">Add comment</button>
        </div>
      </form>
    </div>
  );
}

function CommentEditor({
  comment,
  onCancel,
  onSave,
}: {
  comment: ReviewComment;
  onCancel: () => void;
  onSave: (body: string) => Promise<void>;
}) {
  const [body, setBody] = useState(comment.body);

  return (
    <div class="comment-editor">
      <textarea
        rows={3}
        value={body}
        onInput={(event) => setBody(event.currentTarget.value)}
      />
      <div>
        <button onClick={onCancel}>Cancel</button>
        <button disabled={!body.trim()} onClick={() => void onSave(body)}>Save</button>
      </div>
    </div>
  );
}

function Tree({
  nodes,
  selected,
  onSelect,
}: {
  nodes: TreeNode[];
  selected: string | null;
  onSelect: (path: string) => void;
}) {
  return (
    <ul class="tree">
      {nodes.map((node) => (
        <li key={node.path}>
          {node.kind === "directory" ? (
            <details open>
              <summary>{node.name}</summary>
              <Tree nodes={node.children ?? []} selected={selected} onSelect={onSelect} />
            </details>
          ) : (
            <button
              class={selected === node.path ? "selected" : undefined}
              onClick={() => onSelect(node.path)}
            >
              {node.name}
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

function firstFile(nodes: TreeNode[]): string | null {
  for (const node of nodes) {
    if (node.kind === "file") return node.path;
    const nested = firstFile(node.children ?? []);
    if (nested) return nested;
  }
  return null;
}

function treeContains(nodes: TreeNode[], path: string): boolean {
  return nodes.some(
    (node) => node.path === path || (node.children ? treeContains(node.children, path) : false),
  );
}

function initialTheme(): ThemeId {
  const stored = storedPreference(THEME_KEY);
  if (THEMES.some((theme) => theme.id === stored)) return stored as ThemeId;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "forest" : "paper";
}

function storedPreference(key: string): string | null {
  const cookie = window.document.cookie
    .split("; ")
    .find((entry) => entry.startsWith(`${key}=`));
  return cookie ? decodeURIComponent(cookie.slice(key.length + 1)) : window.localStorage.getItem(key);
}

function rememberPreference(key: string, value: string) {
  window.localStorage.setItem(key, value);
  window.document.cookie = `${key}=${encodeURIComponent(value)}; Path=/; Max-Age=31536000; SameSite=Strict`;
}

function closestRenderedLocation(
  root: HTMLElement | null,
  sourceOffset: number,
): HTMLElement | null {
  if (!root) return null;
  let closest: HTMLElement | null = null;
  let closestDistance = Number.POSITIVE_INFINITY;

  for (const candidate of root.querySelectorAll<HTMLElement>(
    "[data-source-start][data-source-end]",
  )) {
    const start = Number(candidate.dataset.sourceStart);
    const end = Number(candidate.dataset.sourceEnd);
    if (!Number.isFinite(start) || !Number.isFinite(end)) continue;
    const distance = sourceOffset < start
      ? start - sourceOffset
      : sourceOffset >= end
        ? sourceOffset - end + 0.5
        : 0;
    if (distance < closestDistance) {
      closest = candidate;
      closestDistance = distance;
    }
  }

  return closest;
}
