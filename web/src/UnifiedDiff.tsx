import { createHunks, diffLines, highlightIntraline } from "./lineDiff";

export function UnifiedDiff({ before, after }: { before: string; after: string }) {
  const hunks = createHunks(diffLines(before, after));
  if (!hunks.length) return <p class="no-diff">No textual changes.</p>;

  return (
    <div class="unified-diff" aria-label="Changed lines">
      {hunks.map((hunk) => (
        <section class="diff-hunk" key={hunk.header}>
          <div class="diff-hunk-header">{hunk.header}</div>
          {highlightIntraline(hunk.lines).map((line, index) => (
            <div class={`diff-line diff-${line.kind}`} key={`${hunk.header}-${index}`}>
              <span class="diff-line-number">{line.oldLine}</span>
              <span class="diff-line-number">{line.newLine}</span>
              <span class="diff-marker">
                {line.kind === "addition" ? "+" : line.kind === "deletion" ? "−" : " "}
              </span>
              <span class="diff-content">
                {line.segments.map((segment, segmentIndex) => (
                  <span class={segment.changed ? "diff-emphasis" : undefined} key={segmentIndex}>
                    {segment.text}
                  </span>
                ))}
                {!line.text && " "}
              </span>
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}
