# Execution Plan

Implementation is underway. The current MVP completes the main vertical slice:
folder browsing, source-mapped rendering, comments, durable storage, generated
agent prompts, task retrieval/submission, conservative re-anchoring, candidate
comparison, and human acceptance. The phases below remain the delivery checklist.

## Guiding sequence

The highest-risk part is not the folder tree or the comment form. It is mapping
an arbitrary selection in rendered Markdown back to durable source locations.
The execution order proves that risk with fixtures before investing in polish.

## Phase 0: Freeze contracts and fixtures

### Work

- Convert the illustrative review data into a versioned JSON Schema.
- Define source offset, line, column, canonical rendered-text, context, and hash
  semantics precisely.
- Define the review-cycle, agent-disposition, and managed `AGENTS.md` contracts.
- Create a fixture corpus covering headings, paragraphs, repeated text,
  emphasis, links, images, inline code, fenced code, lists, block quotes, tables,
  task lists, footnotes if supported, CRLF, emoji, combining characters, and
  non-Latin scripts.
- Record expected source ranges for representative selections.
- Decide final ignore defaults and maximum document size.

### Exit criteria

- Schema examples validate.
- Every offset unit is unambiguous.
- The fixture corpus includes selections within and across inline nodes.
- The agent task and submission examples account for every requested comment ID.

## Phase 1: Source-mapped rendering spike

### Work

- Scaffold the Preact/TypeScript frontend and Markdown parser.
- Build the custom AST renderer with source metadata on addressable leaves.
- Convert a browser selection to rendered quote, source quote, source range,
  line/column range, and context.
- Render a temporary debug panel that exposes the computed anchor.
- Prove re-rendering an anchor as a highlight.

### Exit criteria

- All Phase 0 fixture selections round-trip to the correct Markdown region.
- Selections spanning bold text, links, code, and multiple inline nodes work.
- Unicode and CRLF cases produce the specified byte and line/column values.
- Unsupported selections fail visibly rather than producing a wrong anchor.

### Review gate

If precise mapping requires unacceptable renderer complexity, narrow v1 to
block-scoped comments before continuing. Do not hide this limitation behind
fuzzy matching.

## Phase 2: Local application foundation

### Work

- Create the Rust command and local Axum HTTP server.
- Embed the production frontend.
- Bind to loopback, generate the launch token, and open the browser.
- Add canonical project-root path validation and safe ignore handling.
- Implement project scanning, pruned tree responses, and Markdown reads.
- Add the file watcher and SSE invalidation channel.

### Exit criteria

- `mdreview .` opens the project in a browser from one executable.
- Nested Markdown files appear in a pruned tree.
- Changes, additions, removals, and straightforward renames update the UI.
- Traversal and symlink escape tests pass.
- The production runtime does not require Node.js.

## Phase 3: Viewer experience

### Work

- Build the three-region responsive layout.
- Add Markdown styling, code blocks, tables, themes, heading anchors, loading,
  empty, and error states.
- Add filename quick-open and core keyboard interactions.
- Persist project-local UI preferences in browser storage.
- Add content security policy and raw-HTML protections.

### Exit criteria

- Reading is comfortable on desktop and narrow screens.
- A 1,000-file fixture project remains responsive.
- Links, unsafe URLs, and raw HTML behave according to the security policy.

## Phase 4: Comment workflow and persistence

### Work

- Implement the selection action, composer, highlights, and comment rail.
- Implement create, edit, resolve, reopen, and confirmed delete.
- Add the **Send to agent** scope picker, deterministic prompt preview, optional
  one-time note, and clipboard action.
- Add stable comment and document IDs.
- Implement `.md-review/review.json`, `SCHEMA.md`, atomic writes, locking, and
  schema migration plumbing.
- Implement the read-only `comments` CLI and validated `resolve` command.
- Implement safe, idempotent root `AGENTS.md` integration with explicit approval
  for existing files and no duplicate project-context questionnaire.
- Add optimistic revision checks between selection and comment creation.

### Exit criteria

- A comment survives browser and process restarts.
- Every comment is locatable from both the UI and JSON/CLI output.
- Concurrent app instances cannot silently overwrite review data.
- A comment created against a stale document revision is rejected cleanly.
- An agent can discover the active review workflow from the generated
  instructions, then use existing repository context without changing unrelated
  project guidance.
- A copied prompt identifies its exact task and remains usable in a fresh agent
  session without copying the full document into chat.

## Phase 5: Iteration and re-anchoring

### Work

- Persist deduplicated snapshots only when referenced by a comment.
- Implement review-cycle start, agent task export, candidate submission, and
  reviewer acceptance/reopen actions.
- Implement stable watcher debouncing and content revision detection.
- Implement unchanged diff mapping, exact quote/context matching, candidate
  scoring, and the four anchor-health states.
- Add needs-review and orphaned UI, original-context display, and manual
  reattachment.
- Detect exact-content renames and retain previous paths.
- Add safe snapshot garbage collection rules.

### Exit criteria

- Inserting, removing, or moving unrelated paragraphs preserves correct anchors.
- Repeated selected phrases use context and never attach ambiguously without a
  warning.
- Changing or deleting selected text retains the open comment as needs-review or
  orphaned.
- A save never alters comment workflow status.
- Renaming an unchanged file preserves comments.
- Intermediate file saves do not create named review revisions.
- Candidate submission rejects missing dispositions and leaves claimed changes
  addressed, not resolved, until reviewer acceptance.

## Phase 6: Packaging and release readiness

### Work

- Add cross-platform builds for Linux, macOS, and Windows.
- Add `--version`, `--no-open`, `--include-hidden`, and diagnostic logging.
- Write installation, usage, metadata/version-control, recovery, and LLM workflow
  documentation.
- Add backup and migration failure recovery tests.
- Measure executable size, startup time, tree scan time, parse time, and memory.
- Run accessibility checks for keyboard focus, selection action, popover, color,
  and screen-reader labels.

### Exit criteria

- A clean machine can run the single executable and review a project.
- Corrupt review data yields a recovery path and does not overwrite the source.
- Release checks pass on all target platforms.
- The measured footprint is documented rather than described only as
  "lightweight."

## Test strategy

### Unit tests

- Path sandbox and ignore rules.
- Offset conversion and line/column calculation.
- AST-to-rendered-text mapping.
- Diff boundary mapping and quote/context scoring.
- Comment state transitions and schema migrations.

### Browser component tests

- Selection action positioning and keyboard access.
- Comment composer lifecycle.
- Highlight and rail synchronization.
- Needs-review and orphaned states.
- Send-to-agent scope selection, prompt preview, copy success, and clipboard
  failure fallback.

### End-to-end tests

- Launch against a temporary project, browse, comment, restart, and resolve.
- Modify a file externally and verify each re-anchoring outcome.
- Rename and delete documents.
- Exercise repeated phrases, Unicode, CRLF, and cross-node selections.
- Attempt traversal, stale writes, unsafe Markdown, and concurrent writers.

### Manual review scenarios

- Read a long design document for at least twenty minutes and evaluate visual
  fatigue and comment navigation.
- Give the generated review data to an LLM and verify it can find the file,
  passage, and requested change.
- Start a fresh agent with only the repository path and verify `AGENTS.md` leads
  it to all requested comment IDs and the submission protocol while normal
  project context comes from the repository.
- Apply an LLM edit, reload the file, and inspect every anchor-health outcome.

## Suggested issue breakdown

Each item should be independently reviewable:

1. Repository tooling and fixture corpus.
2. Markdown AST renderer.
3. DOM selection-to-source mapping.
4. Rust server, embedded assets, and launch flow.
5. Root-safe scanner and project tree.
6. File watcher and SSE updates.
7. Viewer layout and Markdown styling.
8. Comment schema and atomic store.
9. Selection composer, highlights, and rail.
10. Comment lifecycle CLI/API.
11. Minimal review-protocol `AGENTS.md` integration.
12. Send-to-agent task creation, prompt preview, and clipboard handoff.
13. Review-cycle task export and candidate submission.
14. Revision snapshots and diff mapping.
15. Context matching and anchor-health UI.
16. Rename and orphan recovery.
17. Performance, accessibility, packaging, and documentation.

## Definition of v1 done

v1 is done only when the full external-edit loop is reliable:

```text
open project -> select rendered text -> comment -> start review cycle
-> context-free agent reads AGENTS.md and produces a candidate
-> agent accounts for every requested comment ID
-> reviewer compares baseline and candidate
-> accept addressed comments or reopen them against the candidate
```

The product is not done if it merely stores line numbers correctly on the first
render. Iteration durability is part of the MVP, not a later enhancement.

## Remaining work after the current MVP

- Expand source-mapping fixtures and browser tests across every supported
  Markdown construct.
- Add safe opt-in root `AGENTS.md` integration.
- Replace polling with native watcher invalidation and SSE where it improves
  responsiveness without increasing runtime weight significantly.
- Add comment editing/deletion and richer line-oriented diff presentation.
- Complete accessibility, cross-platform packaging, and recovery testing.
