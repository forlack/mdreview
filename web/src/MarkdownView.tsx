import type { ComponentChildren, VNode } from "preact";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import type { Root } from "mdast";

import { api } from "./api";
import { embeddedContentPosition, resolveProjectPath } from "./markdownSource";
import { byteToCodeUnit } from "./selection";
import type { ReviewComment } from "./types";

interface MdNode {
  type: string;
  value?: string;
  depth?: number;
  ordered?: boolean;
  start?: number | null;
  checked?: boolean | null;
  url?: string;
  title?: string | null;
  alt?: string | null;
  lang?: string | null;
  align?: Array<"left" | "right" | "center" | null>;
  children?: MdNode[];
  position?: {
    start: { offset?: number };
    end: { offset?: number };
  };
}

interface Props {
  source: string;
  revision: string;
  comments: ReviewComment[];
  documentPath: string;
  articleRef: (element: HTMLElement | null) => void;
  onSelection: () => void;
  onNavigate: (path: string) => void;
}

interface HighlightRange {
  id: string;
  start: number;
  end: number;
}

interface RenderContext {
  source: string;
  ranges: HighlightRange[];
  documentPath: string;
  onNavigate: (path: string) => void;
  headingIds: WeakMap<MdNode, string>;
}

export function MarkdownView({
  source,
  revision,
  comments,
  documentPath,
  articleRef,
  onSelection,
  onNavigate,
}: Props) {
  const tree = unified().use(remarkParse).use(remarkGfm).parse(source) as Root;
  const ranges = comments
    .filter(
      (comment) =>
        comment.status !== "resolved" &&
        comment.currentAnchor.revision === revision &&
        (comment.currentAnchor.health === "exact" || comment.currentAnchor.health === "moved"),
    )
    .map((comment) => ({
      id: comment.id,
      start: byteToCodeUnit(source, comment.currentAnchor.startByte),
      end: byteToCodeUnit(source, comment.currentAnchor.endByte),
    }));
  const nodes = (tree as unknown as MdNode).children ?? [];
  const context: RenderContext = {
    source,
    ranges,
    documentPath,
    onNavigate,
    headingIds: headingIdentifiers(nodes),
  };

  return (
    <article
      class="markdown"
      ref={articleRef}
      onPointerUp={onSelection}
      onKeyUp={onSelection}
    >
      {renderChildren(nodes, context)}
    </article>
  );
}

function renderChildren(
  children: MdNode[],
  context: RenderContext,
): ComponentChildren {
  return children.map((child, index) => renderNode(child, index, context));
}

function renderNode(
  node: MdNode,
  key: number,
  context: RenderContext,
): VNode | string | null {
  const children = () => renderChildren(node.children ?? [], context);
  const position = sourcePosition(node);

  switch (node.type) {
    case "text":
      return renderText(node, key, context);
    case "paragraph":
      return <p key={key}>{children()}</p>;
    case "heading": {
      const Tag = `h${node.depth ?? 2}` as keyof preact.JSX.IntrinsicElements;
      const id = context.headingIds.get(node);
      return (
        <Tag key={key} id={id}>
          {children()}
          {id && <a class="heading-anchor" href={`#${id}`} aria-label="Link to this heading">#</a>}
        </Tag>
      );
    }
    case "emphasis":
      return <em key={key}>{children()}</em>;
    case "strong":
      return <strong key={key}>{children()}</strong>;
    case "delete":
      return <del key={key}>{children()}</del>;
    case "inlineCode":
      return (
        <code
          key={key}
          {...annotation(
            embeddedContentPosition(position, context.source, node.value ?? ""),
            context.source,
            node.value ?? "",
            context.ranges,
          )}
        >
          {node.value}
        </code>
      );
    case "code":
      return (
        <pre key={key}>
          <code
            class={node.lang ? `language-${node.lang}` : undefined}
            {...annotation(
              embeddedContentPosition(position, context.source, node.value ?? ""),
              context.source,
              node.value ?? "",
              context.ranges,
            )}
          >
            {node.value}
          </code>
        </pre>
      );
    case "blockquote":
      return <blockquote key={key}>{children()}</blockquote>;
    case "list":
      return node.ordered ? (
        <ol key={key} start={node.start ?? undefined}>{children()}</ol>
      ) : (
        <ul key={key}>{children()}</ul>
      );
    case "listItem":
      return (
        <li key={key} class={node.checked != null ? "task-item" : undefined}>
          {node.checked != null && (
            <input type="checkbox" checked={node.checked} disabled aria-hidden="true" />
          )}
          {children()}
        </li>
      );
    case "link":
      return renderLink(node, key, children(), context);
    case "image": {
      const relative = resolveProjectPath(context.documentPath, node.url);
      return (
        <img
          key={key}
          src={relative ? api.assetUrl(relative) : safeUrl(node.url)}
          alt={node.alt ?? ""}
        />
      );
    }
    case "thematicBreak":
      return <hr key={key} />;
    case "break":
      return <br key={key} />;
    case "table":
      return renderTable(node, key, context);
    case "tableRow":
      return <tr key={key}>{children()}</tr>;
    case "tableCell":
      return <td key={key}>{children()}</td>;
    case "html":
      return null;
    default:
      return node.children ? <span key={key}>{children()}</span> : null;
  }
}

function renderText(
  node: MdNode,
  key: number,
  context: RenderContext,
): VNode {
  const value = node.value ?? "";
  const position = sourcePosition(node);
  if (!position) return <span key={key}>{value}</span>;
  const exact = context.source.slice(position.start, position.end) === value;
  if (!exact) {
    return (
      <span key={key} {...annotation(position, context.source, value, context.ranges)}>
        {value}
      </span>
    );
  }

  const boundaries = new Set([0, value.length]);
  for (const range of context.ranges) {
    const start = Math.max(position.start, range.start);
    const end = Math.min(position.end, range.end);
    if (start < end) {
      boundaries.add(start - position.start);
      boundaries.add(end - position.start);
    }
  }
  const sorted = [...boundaries].sort((a, b) => a - b);

  return (
    <span key={key} class="source-text">
      {sorted.slice(0, -1).map((start, index) => {
        const end = sorted[index + 1];
        const ids = context.ranges
          .filter(
            (range) => range.start < position.start + end && range.end > position.start + start,
          )
          .map((range) => range.id);
        const Tag = ids.length ? "mark" : "span";
        return (
          <Tag
            key={start}
            class={ids.length ? "comment-highlight" : undefined}
            data-comment-ids={ids.join(" ") || undefined}
            data-source-start={position.start + start}
            data-source-end={position.start + end}
            data-source-exact="true"
          >
            {value.slice(start, end)}
          </Tag>
        );
      })}
    </span>
  );
}

function sourcePosition(node: MdNode): { start: number; end: number } | null {
  const start = node.position?.start.offset;
  const end = node.position?.end.offset;
  return start == null || end == null ? null : { start, end };
}

function annotation(
  position: { start: number; end: number } | null,
  source: string,
  rendered: string,
  ranges: HighlightRange[],
) {
  if (!position) return {};
  const ids = ranges
    .filter((range) => range.start < position.end && range.end > position.start)
    .map((range) => range.id);
  return {
    "data-source-start": position.start,
    "data-source-end": position.end,
    "data-source-exact": String(source.slice(position.start, position.end) === rendered),
    "data-comment-ids": ids.join(" ") || undefined,
    class: ids.length ? "comment-highlight" : undefined,
  };
}

function renderLink(
  node: MdNode,
  key: number,
  children: ComponentChildren,
  context: RenderContext,
): VNode {
  if (node.url?.startsWith("#")) {
    return <a key={key} href={node.url} title={node.title ?? undefined}>{children}</a>;
  }

  const relative = resolveProjectPath(context.documentPath, node.url);
  if (relative && isMarkdownPath(relative)) {
    return (
      <a
        key={key}
        href={node.url}
        title={node.title ?? undefined}
        onClick={(event) => {
          event.preventDefault();
          context.onNavigate(relative);
        }}
      >
        {children}
      </a>
    );
  }

  return (
    <a
      key={key}
      href={relative ? api.assetUrl(relative) : safeUrl(node.url)}
      title={node.title ?? undefined}
      rel="noreferrer"
      target="_blank"
    >
      {children}
    </a>
  );
}

function renderTable(node: MdNode, key: number, context: RenderContext): VNode {
  const rows = node.children ?? [];
  const header = rows[0];
  const body = rows.slice(1);
  const renderRow = (row: MdNode, rowKey: number, headerCells: boolean) => (
    <tr key={rowKey}>
      {(row.children ?? []).map((cell, cellIndex) => {
        const Tag = headerCells ? "th" : "td";
        return (
          <Tag key={cellIndex} style={{ textAlign: node.align?.[cellIndex] ?? undefined }}>
            {renderChildren(cell.children ?? [], context)}
          </Tag>
        );
      })}
    </tr>
  );

  return (
    <div class="table-wrap" key={key}>
      <table>
        {header && <thead>{renderRow(header, 0, true)}</thead>}
        <tbody>{body.map((row, index) => renderRow(row, index + 1, false))}</tbody>
      </table>
    </div>
  );
}

function headingIdentifiers(nodes: MdNode[]): WeakMap<MdNode, string> {
  const identifiers = new WeakMap<MdNode, string>();
  const counts = new Map<string, number>();
  const visit = (node: MdNode) => {
    if (node.type === "heading") {
      const base = slugify(plainText(node)) || "section";
      const count = counts.get(base) ?? 0;
      counts.set(base, count + 1);
      identifiers.set(node, count ? `${base}-${count}` : base);
    }
    node.children?.forEach(visit);
  };
  nodes.forEach(visit);
  return identifiers;
}

function plainText(node: MdNode): string {
  return node.value ?? node.children?.map(plainText).join("") ?? "";
}

function slugify(value: string): string {
  return value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}\s-]/gu, "")
    .trim()
    .replace(/[\s-]+/g, "-");
}

function isMarkdownPath(path: string): boolean {
  return /\.(md|markdown)$/i.test(path);
}

function safeUrl(url?: string): string | undefined {
  if (!url) return undefined;
  const trimmed = url.trim().toLowerCase();
  if (
    trimmed.startsWith("https:") ||
    trimmed.startsWith("http:") ||
    trimmed.startsWith("mailto:") ||
    trimmed.startsWith("data:image/")
  ) {
    return url;
  }
  return undefined;
}
