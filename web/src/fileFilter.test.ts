import { describe, expect, it } from "vitest";

import { filterFileTree, fuzzyMatch } from "./fileFilter";
import type { TreeNode } from "./types";

const tree: TreeNode[] = [
  {
    name: "docs",
    path: "docs",
    kind: "directory",
    children: [
      { name: "architecture.md", path: "docs/architecture.md", kind: "file" },
      { name: "product-plan.md", path: "docs/product-plan.md", kind: "file" },
    ],
  },
  { name: "README.md", path: "README.md", kind: "file" },
];

describe("fuzzyMatch", () => {
  it("matches non-contiguous characters in order", () => {
    expect(fuzzyMatch("docs/architecture.md", "darch")).toBe(true);
    expect(fuzzyMatch("docs/architecture.md", "dchart")).toBe(false);
  });
});

describe("filterFileTree", () => {
  it("keeps the parent folders of matching Markdown files", () => {
    expect(filterFileTree(tree, "arch")).toEqual([
      {
        name: "docs",
        path: "docs",
        kind: "directory",
        children: [
          { name: "architecture.md", path: "docs/architecture.md", kind: "file" },
        ],
      },
    ]);
  });

  it("supports fuzzy path matching and space-separated terms", () => {
    expect(filterFileTree(tree, "dcs plan")[0]?.children?.[0]?.name).toBe("product-plan.md");
    expect(filterFileTree(tree, "readme")[0]?.path).toBe("README.md");
  });

  it("returns the original tree when the query is blank", () => {
    expect(filterFileTree(tree, "  ")).toBe(tree);
  });
});
