# Testing

## Automated checks

Run the complete local check set with:

```bash
make test
```

This runs frontend unit tests, a production frontend build, Rust unit tests, and
Clippy with warnings treated as errors. Dependency advisories can be checked
with `npm --prefix web audit`.

## End-to-end fixture

The repository includes `testdata/demo` with plain text, emphasis, a table,
lists, a block quote, and fenced code.

```bash
make build
./target/release/mdreview testdata/demo
```

The end-to-end acceptance path is:

1. Open a nested Markdown file.
2. Select rendered text and add a comment.
3. Confirm `.md-review/review.json` contains the correct UTF-8 byte range,
   one-based line/column range, rendered quote, source quote, context, and hash.
4. Click **Send to agent** and copy the generated prompt.
5. Retrieve the compact task instructions with `mdreview revise <task-id>`.
6. Edit the Markdown and submit with `mdreview review submit <task-id>
   --addressed-all`. Also exercise `--report` for a mixed-result task.
7. Confirm the running viewer discovers the external change, re-anchors the
   unchanged quote, displays the agent response, and exposes **Review changes**.
8. Confirm the comparison contains both the baseline and candidate snapshots.
9. Accept the response and confirm the comment becomes resolved and the review
   task becomes complete.

Additional exercised cases:

- selection inside emphasized Markdown maps to the text inside the delimiters;
- project-wide Send to agent excludes comments already assigned to active tasks;
- editing and confirmed deletion persist correctly;
- API access without the per-launch token returns HTTP 401;
- existing `AGENTS.md` content is unchanged unless `mdreview init --append` is
  explicitly used;
- running `mdreview init` again updates only its marked block.
- refreshing restores the selected Markdown file and its independent document
  scroll position;
- the post-submission diff shows changed lines only, preserves line numbers
  while wrapping, and emphasizes the exact changed text within a line;
- reopening and accepting addressed comments preserve access to task history
  and its candidate diff.

The controlled agent-flow benchmark and its current baseline are documented in
[`REVISION_BENCHMARK.md`](REVISION_BENCHMARK.md).

## Current limitations to target next

- Browser automation is currently run locally rather than checked into CI.
- Project changes are detected by lightweight polling rather than native watcher
  events.
- Manual anchor reattachment UI is not implemented yet.
