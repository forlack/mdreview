# Revision Workflow Benchmark

This benchmark measures the cost of handing the same anchored Markdown comments
to a coding agent. It uses the controlled repository fixture in
`testdata/revision-benchmark` and three comments covering a local-only claim,
human acceptance, and the one-command handoff benefit.

Use these comments verbatim so later results remain comparable:

| Selected text | Request |
| --- | --- |
| `a browser for Markdown documents` | Explain that it runs locally and project content never leaves the machine. |
| `returns the result for review` | State explicitly that the human reviewer accepts or reopens each addressed comment. |
| `The handoff process is efficient` | Replace this vague claim with the concrete benefit: the agent receives all anchored comments through one command. |

## Current result

One real non-interactive Codex run was recorded for each protocol on the same
machine and fixture. These are directional measurements, not a statistically
significant performance study.

| Measure | Report-based baseline | Compact workflow | Change |
| --- | ---: | ---: | ---: |
| `mdreview revise` payload | 6,940 bytes | 1,288 bytes | -81% |
| Payload words | 650 | 175 | -73% |
| Payload lines | 148 | 36 | -76% |
| Wall time | 30.0 s | 24.9 s | -17% |
| Uncached input tokens | 25,895 | 9,043 | -65% |
| Output tokens | 1,006 | 767 | -24% |
| Agent-created report files | 1 | 0 | -100% |
| Correct source revision | pass | pass | unchanged |
| Awaiting-review diff/UI | pass | pass | unchanged |

The compact workflow prints only repository instructions, document paths,
locations, selected text, requests, and the exact submission choice. The common
case ends with:

```text
mdreview review submit <task-id> --addressed-all
```

Agents use `--report <file>` only when a request is not addressed or needs
clarification. This preserves the expressive mixed-result path without making
every successful revision generate and validate bookkeeping JSON.

## Repeat the benchmark

1. Copy `testdata/revision-benchmark` to a temporary directory and initialize a
   Git repository there.
2. Run mdreview against the copy and create the three anchored comments listed
   above.
3. Send the comments to an agent and save the output of
   `mdreview revise <task-id>` plus the agent's JSON event stream and wall time.
4. Verify the agent changed only `guide.md`, all three requests are present,
   submission reaches `awaiting_review`, and the browser diff still has
   changed-only hunks, aligned wrapped line numbers, and intraline emphasis.
5. Reopen one addressed comment, accept another, and confirm **View changes**
   still opens the same candidate diff.

Keep the fixture and comment wording unchanged when comparing later protocol
revisions. Record several runs if the goal is to measure small performance
changes rather than large payload reductions.
