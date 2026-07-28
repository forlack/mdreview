export interface SourcePosition {
  start: number;
  end: number;
}

export function embeddedContentPosition(
  position: SourcePosition | null,
  source: string,
  rendered: string,
): SourcePosition | null {
  if (!position || !rendered) return null;
  const segment = source.slice(position.start, position.end);
  const offset = segment.indexOf(rendered);
  if (offset < 0) return position;
  return {
    start: position.start + offset,
    end: position.start + offset + rendered.length,
  };
}

export function resolveProjectPath(documentPath: string, target?: string): string | null {
  if (!target || target.startsWith("#") || target.startsWith("/") || target.startsWith("\\")) {
    return null;
  }
  if (/^[a-z][a-z\d+.-]*:/i.test(target)) return null;

  const rawPath = target.split(/[?#]/, 1)[0];
  let decoded: string;
  try {
    decoded = decodeURIComponent(rawPath);
  } catch {
    return null;
  }
  const parts = documentPath.split("/").slice(0, -1);
  for (const part of decoded.replaceAll("\\", "/").split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      if (!parts.length) return null;
      parts.pop();
    } else {
      parts.push(part);
    }
  }
  return parts.join("/") || null;
}
