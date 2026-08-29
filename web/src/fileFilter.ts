import type { TreeNode } from "./types";

export function filterFileTree(nodes: TreeNode[], query: string): TreeNode[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return nodes;

  return nodes.flatMap((node) => {
    if (node.kind === "file") {
      const path = node.path.toLocaleLowerCase();
      return terms.every((term) => fuzzyMatch(path, term)) ? [node] : [];
    }

    const children = filterFileTree(node.children ?? [], query);
    return children.length > 0 ? [{ ...node, children }] : [];
  });
}

export function fuzzyMatch(value: string, query: string): boolean {
  let queryIndex = 0;
  for (const character of value) {
    if (character === query[queryIndex]) queryIndex += 1;
    if (queryIndex === query.length) return true;
  }
  return query.length === 0;
}
