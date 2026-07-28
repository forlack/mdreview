export type DiffKind = "context" | "addition" | "deletion";

export interface DiffLine {
  kind: DiffKind;
  text: string;
  oldLine: number | null;
  newLine: number | null;
  oldPosition: number;
  newPosition: number;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface DiffSegment {
  text: string;
  changed: boolean;
}

export interface HighlightedDiffLine extends DiffLine {
  segments: DiffSegment[];
}

interface Edit {
  kind: DiffKind;
  text: string;
}

export function diffLines(before: string, after: string): DiffLine[] {
  const oldLines = splitLines(before);
  const newLines = splitLines(after);
  const edits = myers(oldLines, newLines);
  let oldLine = 1;
  let newLine = 1;

  return edits.map((edit) => {
    const line: DiffLine = {
      ...edit,
      oldLine: edit.kind === "addition" ? null : oldLine,
      newLine: edit.kind === "deletion" ? null : newLine,
      oldPosition: oldLine,
      newPosition: newLine,
    };
    if (edit.kind !== "addition") oldLine += 1;
    if (edit.kind !== "deletion") newLine += 1;
    return line;
  });
}

export function createHunks(lines: DiffLine[], context = 3): DiffHunk[] {
  const changes = lines
    .map((line, index) => (line.kind === "context" ? -1 : index))
    .filter((index) => index >= 0);
  if (!changes.length) return [];

  const ranges: Array<{ start: number; end: number }> = [];
  for (const index of changes) {
    const start = Math.max(0, index - context);
    const end = Math.min(lines.length, index + context + 1);
    const previous = ranges.at(-1);
    if (previous && start <= previous.end) {
      previous.end = Math.max(previous.end, end);
    } else {
      ranges.push({ start, end });
    }
  }

  return ranges.map(({ start, end }) => {
    const hunkLines = lines.slice(start, end);
    const oldCount = hunkLines.filter((line) => line.kind !== "addition").length;
    const newCount = hunkLines.filter((line) => line.kind !== "deletion").length;
    const first = hunkLines[0];
    const oldStart = oldCount === 0 ? Math.max(0, first.oldPosition - 1) : first.oldPosition;
    const newStart = newCount === 0 ? Math.max(0, first.newPosition - 1) : first.newPosition;
    return {
      header: `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`,
      lines: hunkLines,
    };
  });
}

export function highlightIntraline(lines: DiffLine[]): HighlightedDiffLine[] {
  const highlighted = lines.map((line) => ({
    ...line,
    segments: [{ text: line.text, changed: false }],
  }));

  let start = 0;
  while (start < lines.length) {
    if (lines[start].kind === "context") {
      start += 1;
      continue;
    }
    let end = start;
    while (end < lines.length && lines[end].kind !== "context") end += 1;
    const deletions = range(start, end).filter((index) => lines[index].kind === "deletion");
    const additions = range(start, end).filter((index) => lines[index].kind === "addition");
    const pairs = Math.min(deletions.length, additions.length);

    for (let index = 0; index < pairs; index += 1) {
      const deletion = deletions[index];
      const addition = additions[index];
      const [oldSegments, newSegments] = changedSegments(
        lines[deletion].text,
        lines[addition].text,
      );
      highlighted[deletion].segments = oldSegments;
      highlighted[addition].segments = newSegments;
    }
    for (const index of [...deletions.slice(pairs), ...additions.slice(pairs)]) {
      highlighted[index].segments = [{ text: lines[index].text, changed: true }];
    }
    start = end;
  }

  return highlighted;
}

function splitLines(value: string): string[] {
  if (!value) return [];
  return value.replaceAll("\r\n", "\n").split("\n");
}

function myers(oldLines: string[], newLines: string[]): Edit[] {
  const oldLength = oldLines.length;
  const newLength = newLines.length;
  const maximum = oldLength + newLength;
  let frontier = new Map<number, number>([[1, 0]]);
  const trace: Array<Map<number, number>> = [];

  for (let depth = 0; depth <= maximum; depth += 1) {
    trace.push(new Map(frontier));
    for (let diagonal = -depth; diagonal <= depth; diagonal += 2) {
      const moveDown =
        diagonal === -depth ||
        (diagonal !== depth && valueAt(frontier, diagonal - 1) < valueAt(frontier, diagonal + 1));
      let x = moveDown
        ? valueAt(frontier, diagonal + 1)
        : valueAt(frontier, diagonal - 1) + 1;
      let y = x - diagonal;

      while (x < oldLength && y < newLength && oldLines[x] === newLines[y]) {
        x += 1;
        y += 1;
      }
      frontier.set(diagonal, x);
      if (x >= oldLength && y >= newLength) {
        return backtrack(trace, oldLines, newLines);
      }
    }
  }

  return [];
}

function backtrack(
  trace: Array<Map<number, number>>,
  oldLines: string[],
  newLines: string[],
): Edit[] {
  const edits: Edit[] = [];
  let x = oldLines.length;
  let y = newLines.length;

  for (let depth = trace.length - 1; depth >= 0; depth -= 1) {
    const frontier = trace[depth];
    const diagonal = x - y;
    const previousDiagonal =
      diagonal === -depth ||
      (diagonal !== depth &&
        valueAt(frontier, diagonal - 1) < valueAt(frontier, diagonal + 1))
        ? diagonal + 1
        : diagonal - 1;
    const previousX = valueAt(frontier, previousDiagonal);
    const previousY = previousX - previousDiagonal;

    while (x > previousX && y > previousY) {
      edits.push({ kind: "context", text: oldLines[x - 1] });
      x -= 1;
      y -= 1;
    }
    if (depth === 0) break;
    if (x === previousX) {
      edits.push({ kind: "addition", text: newLines[y - 1] });
      y -= 1;
    } else {
      edits.push({ kind: "deletion", text: oldLines[x - 1] });
      x -= 1;
    }
  }

  return edits.reverse();
}

function valueAt(frontier: Map<number, number>, diagonal: number): number {
  return frontier.get(diagonal) ?? 0;
}

function range(start: number, end: number): number[] {
  return Array.from({ length: end - start }, (_, index) => start + index);
}

function changedSegments(before: string, after: string): [DiffSegment[], DiffSegment[]] {
  const oldCharacters = [...before];
  const newCharacters = [...after];
  let prefix = 0;
  while (
    prefix < oldCharacters.length &&
    prefix < newCharacters.length &&
    oldCharacters[prefix] === newCharacters[prefix]
  ) {
    prefix += 1;
  }

  let suffix = 0;
  while (
    suffix < oldCharacters.length - prefix &&
    suffix < newCharacters.length - prefix &&
    oldCharacters[oldCharacters.length - suffix - 1] ===
      newCharacters[newCharacters.length - suffix - 1]
  ) {
    suffix += 1;
  }

  return [
    segmentsFor(oldCharacters, prefix, suffix),
    segmentsFor(newCharacters, prefix, suffix),
  ];
}

function segmentsFor(characters: string[], prefix: number, suffix: number): DiffSegment[] {
  const segments = [
    { text: characters.slice(0, prefix).join(""), changed: false },
    {
      text: characters.slice(prefix, characters.length - suffix).join(""),
      changed: true,
    },
    {
      text: suffix ? characters.slice(characters.length - suffix).join("") : "",
      changed: false,
    },
  ];
  return segments.filter((segment) => segment.text.length > 0);
}
