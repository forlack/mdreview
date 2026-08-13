# Proposed Generated AGENTS.md Content

This is the minimal managed block that `mdreview init` would offer to place in a
project-root `AGENTS.md`. It is a product template, not active instructions for
developing this repository.

```markdown
<!-- md-review:managed:start -->
## Markdown review workflow

When asked to revise Markdown from review comments, do not rely on prior chat
context.

Run `mdreview revise <task-id>` using the task ID in the handoff prompt. This
returns the anchored comments and exact submission command. Follow all other
repository instructions and use the repository itself for project context.

Edit the Markdown source, preserve unrelated content, and run applicable
validation. If every request was handled, submit with the `--addressed-all`
command returned by `mdreview revise`. For mixed results or requests needing
clarification, follow its report instructions instead. Never resolve comments;
the human reviewer accepts or reopens them.
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
