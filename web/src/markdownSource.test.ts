import { describe, expect, it } from "vitest";

import { embeddedContentPosition, resolveProjectPath } from "./markdownSource";

describe("embeddedContentPosition", () => {
  it("maps fenced-code text inside its Markdown delimiters", () => {
    const source = "Before\n```text\nreview -> edit -> verify\n```\nAfter";
    const start = source.indexOf("```");
    const end = source.lastIndexOf("```") + 3;

    expect(embeddedContentPosition({ start, end }, source, "review -> edit -> verify")).toEqual({
      start: source.indexOf("review"),
      end: source.indexOf("verify") + "verify".length,
    });
  });

  it("maps inline-code text inside backticks", () => {
    const source = "Use `mdreview revise` now";
    expect(embeddedContentPosition({ start: 4, end: 23 }, source, "mdreview revise")).toEqual({
      start: 5,
      end: 20,
    });
  });
});

describe("resolveProjectPath", () => {
  it("resolves sibling and parent-relative project paths", () => {
    expect(resolveProjectPath("docs/guide/start.md", "../images/example.png")).toBe(
      "docs/images/example.png",
    );
    expect(resolveProjectPath("docs/guide/start.md", "next.md#details")).toBe(
      "docs/guide/next.md",
    );
  });

  it("rejects paths that escape the project or use external schemes", () => {
    expect(resolveProjectPath("README.md", "../secret.md")).toBeNull();
    expect(resolveProjectPath("README.md", "https://example.com/a.md")).toBeNull();
  });
});
