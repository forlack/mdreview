# Architecture Design

## 1. Recommended architecture

Build a local web application distributed as a single Rust executable.

```text
mdreview <project path>
        |
        v
+------------------------+       HTTP + SSE       +----------------------+
| Local Rust process     | <--------------------> | Browser UI           |
|                        |                        | Preact + TypeScript   |
| - project sandbox      |                        | Markdown AST renderer|
| - lightweight polling  |                        | selection/comment UI |
| - comment store        |                        +----------------------+
| - review cycles        |
| - revision/reanchor    |
| - embedded web assets  |
+------------------------+
        |
        +---- project Markdown files (read-only in v1)
        |
        +---- .md-review/review.json
        |
        +---- .md-review/revisions/<sha256>.md
```

This gives the browser-quality selection and layout needed for annotation while
avoiding Electron's bundled browser. It also avoids making the whole product
depend on browser-specific directory permissions. Browser directory APIs require
explicit grants and have varying support and behavior; a local process gives us
predictable folder traversal, file watching, and atomic storage.

The built frontend is embedded into the Rust binary with `rust-embed`. Node.js
is a build-time dependency only.

## 2. Technology choices

### Backend and packaging

- Axum on Tokio for the local HTTP server.
- `rust-embed` for the compiled frontend.
- A small cross-platform filesystem watcher dependency; polling is a fallback
  for filesystems where native watching is unreliable.
- SHA-256 for revision identity.
- JSON files, not SQLite, so review state remains transparent to an LLM and to
  version control.

### Frontend

- Preact with TypeScript for a small component runtime.
- Vite for development and production builds.
- `remark-parse` and `remark-gfm` to produce a Markdown AST with source
  positions.
- A custom AST-to-Preact renderer for precise source mapping. This is more
  important than using a generic `dangerouslySetInnerHTML` renderer.
- Plain CSS with design tokens; no component framework in v1.

The relevant project documentation supports these choices: Axum provides the
local HTTP layer, `rust-embed` packages built assets into the executable, Preact
supports a Vite build, and unist nodes carry source positions. See References.

## 3. Process and security boundary

The server:

- binds only to `127.0.0.1` on an available port;
- generates a random per-launch token included in the opened URL and required
  for API requests;
- resolves and canonicalizes every requested path beneath the selected root;
- rejects `..`, absolute-path injection, null bytes, and paths that escape the
  root through symlinks;
- does not follow directory symlinks by default;
- serves a restrictive Content Security Policy;
- disables raw Markdown HTML by default;
- never exposes arbitrary filesystem browsing to a remote client.

The command prints the URL and attempts to open the default browser. `Ctrl+C`
stops the server. A `--no-open` option supports headless or remote development.

## 4. Major components

### Project scanner

Walks the root, applies ignore rules, and returns a pruned tree containing only
Markdown files and their ancestor folders. Paths crossing the API are always
root-relative slash-separated paths.

### Document service

Reads Markdown, checks size and UTF-8 validity, computes its revision hash, and
returns the source. v1 should show a clear error rather than attempting to render
binary, non-UTF-8, or unusually large files.

### Markdown renderer and source map

Parses the source into an AST. The renderer wraps addressable text leaves with
metadata connecting DOM text to Markdown source positions. It also builds a
canonical rendered-text stream so a selection spanning multiple styled nodes can
be represented consistently.

### Selection controller

Converts a browser `Range` into:

1. exact selected rendered text;
2. rendered-text offsets;
3. the smallest valid Markdown source range;
4. source line and column endpoints;
5. prefix and suffix context;
6. the document revision hash.

The server verifies that the source excerpt and revision match before storing a
comment. Browser string offsets are UTF-16 code-unit offsets; persisted source
offsets are zero-based UTF-8 byte offsets. Human-facing lines and columns are
one-based and columns count Unicode code points. Conversion is explicit and
covered by Unicode tests.

### Comment service

Validates comments, assigns sortable stable IDs, performs atomic writes, and
maintains workflow status separately from anchor health.

### Review-cycle service

Freezes a commented baseline, exports compact agent instructions, accepts one
candidate revision plus either an all-addressed acknowledgement or a disposition
for every requested comment ID, and keeps the candidate in an awaiting-review
state until the reviewer accepts or reopens the claims. Filesystem watcher
events alone never create a named review cycle.

### Agent prompt generator

Derives a plain-text handoff prompt deterministically from a stored review task.
The prompt includes stable identifiers and retrieval instructions, but not full
document or comment contents. The browser previews it and writes it to the
clipboard only after the user clicks Copy. Prompt generation makes no network or
LLM request and can be repeated without changing the task.

### Revision and re-anchoring service

Stores only source snapshots referenced by comments. On a stable file change it
maps existing anchors forward conservatively and records the result. It does not
keep a snapshot for every editor keystroke and is not a replacement for Git.

### Refresh service

The current implementation polls the project tree, selected document, comments,
and review tasks every three seconds. This keeps the runtime small and works
consistently across platforms. Native file watching remains a possible future
optimization if polling becomes measurable overhead.

## 5. Comment anchor design

Line and character counts alone are not durable. A comment stores several
selectors so each compensates for the weaknesses of the others:

- **Revision selector:** SHA-256 of the source at comment creation.
- **Source position selector:** UTF-8 byte range plus line/column endpoints.
- **Rendered text position selector:** range in a canonical rendered-text
  stream.
- **Text quote selector:** exact selected text plus bounded prefix and suffix.
- **Source quote selector:** exact Markdown source covered by the selection plus
  bounded context.

This follows the useful shape of text-position and text-quote annotation
selectors without requiring a web-annotation server.

### Re-anchoring algorithm

For each active (`open` or `addressed`) or displayed resolved comment after a
file changes:

1. If the revision is unchanged, keep the anchor as `exact`.
2. Diff the stored revision snapshot against current source. If both boundaries
   map through unchanged text and the quote still verifies, attach as `moved`.
3. Search for an exact quote and score bounded prefix, suffix, and proximity to
   the mapped line. A unique high-confidence result attaches as `moved`.
4. If the passage changed but the diff identifies one clear replacement region,
   retain that candidate as `needs_review`; do not present it as an exact
   highlight.
5. Multiple plausible matches become `needs_review` with candidates.
6. No plausible location becomes `orphaned`.

No low-confidence fuzzy or semantic match is silently accepted in v1.

### Comment state transitions

```text
workflow:  open ---- agent claim ----> addressed ---- reviewer accepts ----> resolved
             ^                              |
             +------- reviewer reopens -----+

anchor:    exact --> moved --> needs_review --> orphaned
              ^          manual reattach          |
              +-----------------------------------+
```

A file save can change only anchor health. An agent submission can move an open
comment to addressed. Only reviewer acceptance moves it to resolved.

## 6. Agent discovery

The standard integration point is the project-root `AGENTS.md`, because that file
is discoverable before an agent enters the hidden review directory. It supplies
only review-protocol context. The repository's existing files and instructions
remain authoritative for project context.

`mdreview init` follows these rules:

1. If root `AGENTS.md` is absent, offer to create the minimal review instructions.
2. If it exists, show the exact managed block and require approval before
   inserting it.
3. Delimit the block with stable comments so later updates are idempotent.
4. Never rewrite text outside the managed block.
5. If integration is declined, leave a copyable snippet and report that
   automatic agent discovery is not configured.

The root instructions are intentionally short. They tell the agent to:

- follow all other repository instructions and infer normal project context from
  the repository;
- read `.md-review/review.json` or use `mdreview comments` for active comment
  data;
- submit a candidate while accounting for each requested comment ID;
- leave final resolution to the human reviewer.

See [AGENTS_TEMPLATE.md](AGENTS_TEMPLATE.md) for the proposed generated content.

## 7. Project storage

Proposed layout:

```text
project/
  AGENTS.md
  docs/
    design.md
  .md-review/
    review.json
    SCHEMA.md
    revisions/
      4f62...c91.md
```

`review.json` is the source of truth and is written with stable ordering and
formatting. Writes use a temporary sibling file, file flush, and atomic rename.
A project lock prevents two local server processes from overwriting each other.

Illustrative shape, not a final schema:

```json
{
  "schemaVersion": 1,
  "documents": {
    "d_01K0...": {
      "path": "docs/design.md",
      "previousPaths": [],
      "currentRevision": "sha256:ab12..."
    }
  },
  "comments": [
    {
      "id": "c_01K0...",
      "documentId": "d_01K0...",
      "status": "open",
      "body": "Explain why this constraint exists.",
      "createdAt": "2026-07-25T20:00:00Z",
      "originalAnchor": {
        "revision": "sha256:4f62...",
        "startByte": 418,
        "endByte": 461,
        "startLine": 14,
        "startColumn": 3,
        "endLine": 14,
        "endColumn": 46,
        "renderedExact": "the selected rendered words",
        "sourceExact": "the **selected rendered** words",
        "prefix": "bounded text before ",
        "suffix": " bounded text after"
      },
      "currentAnchor": {
        "revision": "sha256:ab12...",
        "health": "moved",
        "startLine": 18,
        "startColumn": 3,
        "endLine": 18,
        "endColumn": 46
      },
      "resolvedAt": null,
      "resolutionNote": null
    }
  ]
}
```

Review cycles are also stored in `review.json`. A cycle links a baseline and
candidate revision to the exact comment set the agent was asked to handle:

```json
{
  "id": "cycle_01K0...",
  "documentId": "d_01K0...",
  "baseRevision": "sha256:4f62...",
  "candidateRevision": "sha256:ab12...",
  "status": "awaiting_review",
  "requestedCommentIds": ["c_01K0...", "c_01K1..."],
  "dispositions": [
    {
      "commentId": "c_01K0...",
      "result": "addressed",
      "note": "Added the rejected alternative and explained the tradeoff."
    },
    {
      "commentId": "c_01K1...",
      "result": "needs_clarification",
      "note": "The intended audience is not specified."
    }
  ]
}
```

A submitted cycle is invalid if it silently omits a requested comment ID.

The final schema should use a small migration layer from the first release.
Resolved comments retain their original anchor. Snapshots may be pruned only when
no retained comment references them.

### Document rename handling

Comments refer to a document ID rather than only a path. If one watched file
disappears and another appears with the same revision, treat it as a rename and
update `path` while retaining `previousPaths`. If identity is ambiguous, ask for
manual reassociation rather than guessing.

## 8. Local API sketch

```text
GET    /api/project/tree
GET    /api/documents/:documentId
GET    /api/documents/:documentId/comments
POST   /api/documents/:documentId/comments
PATCH  /api/comments/:commentId
DELETE /api/comments/:commentId
POST   /api/comments/:commentId/reattach
POST   /api/review-cycles
GET    /api/review-cycles/:cycleId/prompt
POST   /api/review-cycles/:cycleId/submit
POST   /api/review-cycles/:cycleId/accept
GET    /api/events
```

All mutations use optimistic revision checks. Creating a comment against an old
revision returns a conflict and asks the UI to reload instead of storing a
misplaced anchor.

The prompt endpoint returns plain text derived from the immutable task scope. An
optional one-time user note is applied in the browser when copying and does not
silently modify comment bodies.

## 9. Performance and reliability constraints

- Scan lazily and cache directory results; never parse every Markdown file just
  to construct the tree.
- Parse only the active document and documents needed for re-anchoring.
- Debounce watcher events and wait for a stable readable file before producing a
  new revision.
- Limit context selectors to a fixed size, such as 80 Unicode code points on
  each side.
- Virtualize the comment rail only if measurement shows it is necessary.
- Keep all metadata writes atomic and preserve a last-known-good backup before a
  schema migration.

## 10. Alternatives considered

### Browser-only progressive web app

Attractive because it needs no backend, but directory access, persistent
permissions, file watching, and cross-browser behavior complicate the core local
workflow. It is not the recommended v1 foundation.

### Tauri

Much lighter than Electron and a reasonable later wrapper if a native folder
picker, app icon, and dedicated window are important. It adds packaging and
platform integration work that does not prove the core review interaction.

### Electron

Provides straightforward filesystem and browser integration, but the bundled
runtime conflicts with the lightweight goal.

### SQLite

Strong transactional storage, but opaque to ordinary file-reading LLM workflows
and unnecessary for a single-user v1.

### Inline Markdown comments

HTML comments or custom syntax inside the reviewed document make content diffs
noisy, can interfere with renderers, and couple review state to the deliverable.
Sidecar data is cleaner.

## 11. References

- [Axum](https://github.com/tokio-rs/axum)
- [`rust-embed`](https://github.com/pyros2097/rust-embed)
- [Preact getting started guide](https://preactjs.com/guide/v10/getting-started/)
- [remark](https://github.com/remarkjs/remark)
- [unist positions](https://github.com/syntax-tree/unist#position)
- [MDN File System API overview](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API)
