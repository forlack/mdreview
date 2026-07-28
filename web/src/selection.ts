import type { PendingAnchor } from "./types";

const encoder = new TextEncoder();

export function captureSelection(
  article: HTMLElement,
  source: string,
  revision: string,
): PendingAnchor | null {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return null;

  const range = selection.getRangeAt(0);
  if (!article.contains(range.commonAncestorContainer)) return null;

  const leaves = Array.from(
    article.querySelectorAll<HTMLElement>("[data-source-start][data-source-end]"),
  ).filter((leaf) => {
    try {
      return range.intersectsNode(leaf);
    } catch {
      return false;
    }
  });
  if (leaves.length === 0) return null;

  const first = leaves[0];
  const last = leaves[leaves.length - 1];
  let start = boundaryOffset(range.startContainer, range.startOffset, first, true);
  let end = boundaryOffset(range.endContainer, range.endOffset, last, false);
  if (start > end) [start, end] = [end, start];
  if (start === end) return null;

  const renderedExact = selection.toString().trim();
  if (!renderedExact) return null;

  const rect = range.getBoundingClientRect();
  return {
    revision,
    startByte: encoder.encode(source.slice(0, start)).length,
    endByte: encoder.encode(source.slice(0, end)).length,
    renderedExact,
    x: Math.min(window.innerWidth - 120, Math.max(12, rect.right)),
    y: Math.min(window.innerHeight - 48, Math.max(12, rect.bottom + 8)),
  };
}
function boundaryOffset(
  container: Node,
  offset: number,
  fallback: HTMLElement,
  isStart: boolean,
): number {
  const element =
    container.nodeType === Node.TEXT_NODE
      ? container.parentElement
      : container instanceof HTMLElement
        ? container
        : null;
  const annotated = element?.closest<HTMLElement>(
    "[data-source-start][data-source-end]",
  );
  const leaf = annotated ?? fallback;
  const start = Number(leaf.dataset.sourceStart);
  const end = Number(leaf.dataset.sourceEnd);
  const exact = leaf.dataset.sourceExact === "true";

  if (exact && container.nodeType === Node.TEXT_NODE && leaf.contains(container)) {
    return Math.min(end, start + offset);
  }
  return isStart ? start : end;
}

export function byteToCodeUnit(source: string, targetByte: number): number {
  if (targetByte <= 0) return 0;
  let bytes = 0;
  let units = 0;
  for (const character of source) {
    const width = encoder.encode(character).length;
    if (bytes + width > targetByte) break;
    bytes += width;
    units += character.length;
  }
  return units;
}
