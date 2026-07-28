import { describe, expect, it } from "vitest";

import { createHunks, diffLines, highlightIntraline } from "./lineDiff";

describe("line diff", () => {
  it("tracks replacement line numbers", () => {
    const lines = diffLines("one\ntwo\nthree", "one\nchanged\nthree");
    expect(lines.map(({ kind, text, oldLine, newLine }) => ({ kind, text, oldLine, newLine })))
      .toEqual([
        { kind: "context", text: "one", oldLine: 1, newLine: 1 },
        { kind: "deletion", text: "two", oldLine: 2, newLine: null },
        { kind: "addition", text: "changed", oldLine: null, newLine: 2 },
        { kind: "context", text: "three", oldLine: 3, newLine: 3 },
      ]);
  });

  it("creates separate hunks and omits distant unchanged lines", () => {
    const before = Array.from({ length: 12 }, (_, index) => `line ${index + 1}`).join("\n");
    const after = before.replace("line 2", "second line").replace("line 11", "eleventh line");
    const hunks = createHunks(diffLines(before, after), 1);

    expect(hunks).toHaveLength(2);
    expect(hunks[0].lines.map((line) => line.text)).not.toContain("line 7");
    expect(hunks[1].lines.map((line) => line.text)).not.toContain("line 7");
  });

  it("emphasizes only the changed portion of a replacement", () => {
    const lines = highlightIntraline(
      diffLines("The first version should ship", "The v1 release should ship"),
    );
    const deletion = lines.find((line) => line.kind === "deletion");
    const addition = lines.find((line) => line.kind === "addition");

    expect(deletion?.segments.filter((segment) => segment.changed)).toEqual([
      { text: "first version", changed: true },
    ]);
    expect(addition?.segments.filter((segment) => segment.changed)).toEqual([
      { text: "v1 release", changed: true },
    ]);
  });
});
