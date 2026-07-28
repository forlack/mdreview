# Proposed Generated AGENTS.md Content

This is the minimal managed block that `mdreview init` would offer to place in a
project-root `AGENTS.md`. It is a product template, not active instructions for
developing this repository.

```markdown
<!-- md-review:managed:start -->
## Markdown review workflow

When asked to revise Markdown from review comments, do not rely on prior chat
context.

Before editing:

1. Follow all other repository instructions and use the repository itself for
   project context. The review system does not replace those instructions.
2. Run `mdreview revise <task-id>` using the task ID from the handoff
   prompt. This returns the full task instructions and anchored comments. If the
   command is unavailable, read `.md-review/review.json` and its schema guide.
3. Identify the active review cycle and preserve every requested comment ID in
   your work report.
4. Treat the selected quote and location as evidence, not the entire request.
   Follow the comment body together with the repository's existing context.

While editing:

- Edit the Markdown source, not rendered or generated output.
- Preserve unrelated content and follow project-specific instructions elsewhere
  in this `AGENTS.md`.
- Do not edit comment anchors, revision hashes, or workflow state directly.
- If a request conflicts with another instruction or lacks necessary context,
  report `needs_clarification` instead of guessing.

After editing:

1. Run the repository's normal validation checks when applicable.
2. Submit one disposition for every requested comment ID: `addressed`,
   `not_addressed`, or `needs_clarification`, with a concise explanation.
3. When the CLI is available, submit the candidate with
   `mdreview review submit <cycle-id> --report <report-path>`.
4. Never mark a comment resolved. `addressed` means awaiting reviewer approval;
   only the reviewer resolves it.
<!-- md-review:managed:end -->
```

## Existing AGENTS.md behavior

If a project already has an `AGENTS.md`, the application must preview the managed
block and require approval before insertion. It must preserve all existing
content and use the markers only to update its own block. Declining the insertion
does not prevent review; it means agents must be directed to the review context
through some other project instruction.

The viewer does not ask for a project brief, audience, style guide, terminology,
or validation commands. Those belong in the repository if the project needs
them.
