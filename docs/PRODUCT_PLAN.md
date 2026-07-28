# Product Plan

## 1. Product goal

Make reviewing Markdown as immediate as reviewing a document in a collaborative
editor, without moving project files into a hosted service or requiring a heavy
desktop application.

The core loop is:

1. Open a project folder.
2. Browse only the folders that contain Markdown.
3. Read a rendered document.
4. Select rendered text and add a comment.
5. Let a person or LLM edit the original Markdown.
6. Reopen the document and verify, resolve, or reattach the comment.

## 2. Product principles

### Local first

Markdown and comments remain on the user's machine. The v1 application has no
account, cloud service, telemetry, or database server.

### Lightweight means low operational weight

The release should be a single executable that starts quickly and opens a local
browser tab. A small UI dependency is acceptable if it materially improves the
interaction; a bundled browser runtime is not.

### Review data is inspectable

Comments use documented JSON with stable IDs, document paths, line and column
locations, source offsets, selected text, and nearby context. An LLM can read it
with ordinary file tools. The app remains the preferred writer so it can
validate and migrate the schema.

### A save is not a decision

A file change does not prove that a comment was addressed. The application must
never delete or resolve comments merely because the document changed.

### Prefer an honest orphan to a wrong highlight

When a comment cannot be reattached with high confidence, it remains visible in
a needs-review state with its original quote and location.

### A context-free agent should be productive

An agent must not need the conversation in which review comments were created.
The repository remains the authority for its purpose, audience, style, and
constraints. A small root `AGENTS.md` instruction only explains how to discover
and report Markdown review work.

## 3. v1 scope

### Project and document browsing

- Launch against a folder with `mdreview <path>`; `mdreview .` is the common
  path.
- Show a collapsible tree containing `.md` and `.markdown` files.
- Hide directories that do not lead to Markdown files.
- Ignore `.git`, `.md-review`, `node_modules`, common build directories, hidden
  directories by default, and user-configured patterns.
- Remember the last open document for that project in browser-local state.
- Watch for added, removed, renamed, and changed Markdown files.

### Rendered reading

- Render CommonMark plus the GitHub-Flavored Markdown features people commonly
  use in project documents: tables, task lists, strikethrough, autolinks, and
  fenced code.
- Use readable typography, a constrained content width, light and dark themes,
  heading anchors, and responsive navigation.
- Disable raw HTML by default. If it is enabled later, sanitize it.

### Commenting

- Selecting rendered text shows a small Comment action beside the selection.
  The action is not hover-only, so it remains usable with a keyboard or touch.
- Clicking the action opens a focused comment composer without losing the
  selection.
- The comment body states the requested change. The selected passage, source
  location, and surrounding text are captured automatically.
- Existing comments appear as highlights and in a document comment rail.
- Selecting a comment scrolls to and emphasizes its anchor.
- v1 supports create, edit comment body, resolve, reopen, and delete with
  confirmation.
- Filters show active comments (`open` and `addressed`) by default and can reveal
  resolved or needs-review comments.

### Iterations and comment durability

Every comment has two independent dimensions:

- **Workflow status:** `open`, `addressed`, or `resolved`.
- **Anchor health:** `exact`, `moved`, `needs_review`, or `orphaned`.

When a Markdown file changes:

- Unchanged selected text that merely shifted is reattached and marked `moved`.
- A changed selection remains open and becomes `needs_review`.
- A deleted or unlocatable selection remains open and becomes `orphaned`.
- An agent may mark a comment `addressed`, meaning it claims the candidate
  revision handles the request and is awaiting human verification.
- A resolved comment stays resolved and is hidden by default.
- The user can resolve, reopen, or manually reattach any comment.

There is no automatic clearing on save and no automatic semantic claim that a
change addressed a comment.

### Review cycles

A multi-comment rewrite is recorded as a review cycle rather than a collection
of unrelated file saves:

1. The commented document revision becomes the baseline.
2. An agent receives the baseline and stable comment IDs, then uses the project
   itself for normal repository context.
3. The agent edits the Markdown and submits one candidate revision.
4. The agent records a disposition for every requested comment: `addressed`,
   `not_addressed`, or `needs_clarification`, plus a short note.
5. The viewer presents the baseline-to-candidate diff grouped by comment.
6. The reviewer accepts addressed comments as resolved or reopens them against
   the candidate revision.

Intermediate editor saves update the live file hash but do not become named
review revisions. Baselines and submitted candidates are the meaningful revision
boundaries.

### LLM workflow

The project contains `.md-review/review.json` and a schema guide. Existing
repository files and instructions remain the source of project context.

`mdreview init` offers to create a short root `AGENTS.md` block that tells an
agent where review comments live and how to report its response. It never asks
the user to restate project context and never silently replaces an existing
`AGENTS.md`.

Each open comment exposes:

- a stable comment ID;
- the current relative document path;
- current line and column range;
- selected rendered text;
- source excerpt and surrounding context;
- comment body and status;
- original and current revision hashes;
- anchor health.

The binary also provides deterministic machine-readable commands:

```text
mdreview comments --open --format json
mdreview comments --document docs/design.md --format json
mdreview revise <task-id>
mdreview review submit <cycle-id> --report .md-review/agent-report.json
mdreview resolve <comment-id> --note "Verified in the candidate revision"
```

An LLM may read the JSON directly. Status changes should go through the CLI or
HTTP API when practical so invalid state cannot be written.

The generated agent instructions require the agent to report every requested
comment ID, including requests it could not address. Missing IDs remain open and
are called out by the viewer.

### Send to agent

The comment rail includes a **Send to agent** action. It does not invoke an LLM.
It performs a lightweight handoff:

1. Let the reviewer choose the current document's open comments or selected open
   comments across the project.
2. Create a review task with stable comment IDs and baseline revision hashes.
3. Show a prompt preview with Copy and Cancel actions.
4. Copy the prompt as plain text for pasting into an agent already working in the
   project folder.
5. Keep the task available so the prompt can be copied again later.

The prompt is deterministic and concise. It contains the task ID, affected
relative paths, requested comment IDs, retrieval command, fallback data path,
submission requirement, and the rule that only the reviewer resolves comments.
It does not duplicate document contents or attempt to restate project context.

Example generated prompt:

```text
Address Markdown review task task_01KABC in this repository.

Follow all instructions in AGENTS.md and use the repository for project context.
The task covers 3 comments in docs/plan.md: C-101, C-102, and C-104.

Run `mdreview revise task_01KABC` in this repository and follow the
instructions it returns.
```

The preview allows the user to add a one-time sentence before copying, such as
"Keep this revision especially concise." That note belongs to the task handoff,
not permanent project configuration.

## 4. Primary user experience

The desktop layout has three regions:

```text
+----------------+------------------------------------+------------------+
| Project tree   | Rendered Markdown                  | Comments         |
|                |                                    |                  |
| docs/          |  Architecture                      | 3 open           |
|   plan.md      |                                    |                  |
|   design.md    |  Selected text is highlighted.     | C-102 Clarify... |
| README.md      |                       [Comment]     | C-099 Example... |
+----------------+------------------------------------+------------------+
```

On narrow screens, the project tree and comments become drawers rather than
shrinking the document to an unreadable width.

Keyboard basics for v1:

- `Ctrl/Cmd+P`: find a Markdown file by name.
- `C` while a non-empty selection is active: open the comment composer.
- `Esc`: close the composer or active comment.
- `[` and `]`: previous and next open comment.

## 5. What v1 deliberately does not include

- Editing Markdown in the viewer.
- Cloud synchronization, accounts, or multi-user live collaboration.
- Comment threads, mentions, reactions, or notifications.
- PDF, DOCX, or non-Markdown review.
- Automatic LLM calls or automatic claims that feedback was addressed.
- A complete version-control system for Markdown.
- Electron or a bundled Chromium runtime.
- Semantic/fuzzy reattachment that can silently choose the wrong passage.

## 6. Success criteria

The first release is successful when:

- A project with 1,000 Markdown files opens and filters its tree without a
  noticeable interaction delay on a typical laptop.
- The packaged application has one runtime executable and needs no Node.js or
  database installation.
- A user can create a comment from a rendered selection in two deliberate
  actions: select, then submit the comment.
- Comment anchors are correct across plain text, emphasis, links, inline code,
  headings, lists, block quotes, fenced code, tables, and Unicode text.
- Inserting unrelated text before a comment preserves its highlight after the
  file changes.
- Editing or deleting the selected text never makes the comment disappear.
- An LLM can locate every open comment and its target by reading one documented
  JSON file or invoking one CLI command.
- A new agent can discover open comments and the submission protocol from
  `AGENTS.md`, then use the repository's existing instructions and files for
  project context.
- A candidate rewrite accounts for every requested comment ID and remains
  awaiting review until the user accepts it.
- **Send to agent** produces a usable prompt in one click, and a fresh agent in
  the project can retrieve the referenced task without the prior conversation.
- Restarting the app or browser does not lose review state.

## 7. Decisions for product review

The plan recommends these choices. They are review gates, not implemented facts.

1. **Launch model:** local binary plus normal browser tab.
2. **Project opening:** command path for v1; a native folder picker can come
   later through a small desktop wrapper if it proves valuable.
3. **Storage:** project-local `.md-review/`, suitable for committing when review
   state should travel with the project.
4. **Editing:** external editor or LLM only in v1.
5. **Resolution:** always explicit; never inferred from a save.
6. **Retention:** resolved comments and their original anchor are retained until
   explicitly deleted or compacted.
7. **Agent discovery:** offer a managed root `AGENTS.md` block that points to
   machine-readable comments; never duplicate project context or overwrite
   unrelated agent instructions.
