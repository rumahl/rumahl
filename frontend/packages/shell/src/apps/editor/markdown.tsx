import { Fragment, type ReactNode } from "react";

/**
 * Markdown for the native editor: one parser, three projections.
 *
 * - `MarkdownDocument` renders safe React elements for the preview.
 * - `markdownToHtml` renders the same tree as escaped HTML for the rich
 *   contenteditable surface (the editor shows formatted text, not syntax).
 * - `htmlToMarkdown` serializes the edited DOM back to Markdown on save.
 *
 * Text is always escaped or rendered as React text, so a document can never
 * inject HTML. Unsafe URL schemes are dropped.
 */
export function isMarkdownFile(name: string): boolean {
  return /\.(md|markdown|mdx)$/i.test(name.trim());
}

const SAFE_URL = /^(?:https?:\/\/|mailto:|#|\/|data:image\/)/i;

function safeUrl(raw: string): string | null {
  const url = raw.trim().replace(/"/g, "%22");
  if (!url || url.length > 2048) return null;
  return SAFE_URL.test(url) ? url : null;
}

type Inline =
  | { type: "text"; value: string }
  | { type: "strong"; children: Inline[] }
  | { type: "em"; children: Inline[] }
  | { type: "del"; children: Inline[] }
  | { type: "code"; value: string }
  | { type: "link"; href: string; children: Inline[] }
  | { type: "image"; src: string; alt: string };

type Block =
  | { type: "heading"; level: number; children: Inline[] }
  | { type: "paragraph"; lines: Inline[][] }
  | { type: "code"; language: string; value: string }
  | { type: "list"; ordered: boolean; items: { children: Inline[]; checked: boolean | null }[] }
  | { type: "quote"; children: Block[] }
  | { type: "hr" }
  | { type: "table"; header: Inline[][]; rows: Inline[][][] };

const INLINE = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|\[([^\]]+)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|(`[^`]+`)|(\*\*|__)(.+?)\6|~~(.+?)~~|(\*|_)(.+?)\9/g;

function parseInline(source: string): Inline[] {
  const nodes: Inline[] = [];
  let last = 0;
  for (const match of source.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > last) nodes.push({ type: "text", value: source.slice(last, at) });
    if (match[1] !== undefined) {
      const src = safeUrl(match[2] ?? "");
      if (src) nodes.push({ type: "image", src, alt: match[1] });
      else nodes.push({ type: "text", value: match[1] });
    } else if (match[3] !== undefined) {
      const href = safeUrl(match[4] ?? "");
      const children = parseInline(match[3]);
      nodes.push(href ? { type: "link", href, children } : { type: "text", value: match[3] });
    } else if (match[5] !== undefined) {
      nodes.push({ type: "code", value: match[5].slice(1, -1) });
    } else if (match[6] !== undefined) {
      nodes.push({ type: "strong", children: parseInline(match[7] ?? "") });
    } else if (match[8] !== undefined) {
      nodes.push({ type: "del", children: parseInline(match[8]) });
    } else if (match[9] !== undefined) {
      nodes.push({ type: "em", children: parseInline(match[10] ?? "") });
    }
    last = at + match[0].length;
  }
  if (last < source.length) nodes.push({ type: "text", value: source.slice(last) });
  return nodes;
}

const LIST_ITEM = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const TASK_ITEM = /^\[([ xX])\]\s+(.*)$/;

function splitRow(line: string): string[] {
  return line.trim().replace(/^\|/, "").replace(/\|$/, "").split(/(?<!\\)\|/).map((cell) => cell.trim().replace(/\\\|/g, "|"));
}

function isBlockStart(line: string): boolean {
  return /^\s{0,3}(#{1,6})\s+/.test(line)
    || /^\s{0,3}(```|~~~)/.test(line)
    || /^\s{0,3}>\s?/.test(line)
    || LIST_ITEM.test(line)
    || /^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)
    || /^\s*\|.*\|\s*$/.test(line);
}

/** Parses Markdown source into the intermediate block tree. */
export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index]!;
    if (!line.trim()) { index += 1; continue; }

    const fence = /^\s{0,3}(```|~~~)\s*([\w-]*)\s*$/.exec(line);
    if (fence) {
      const marker = fence[1]!;
      const body: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index]!.trimStart().startsWith(marker)) { body.push(lines[index]!); index += 1; }
      index += 1;
      blocks.push({ type: "code", language: fence[2] ?? "", value: body.join("\n") });
      continue;
    }

    const heading = /^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (heading) {
      blocks.push({ type: "heading", level: heading[1]!.length, children: parseInline(heading[2] ?? "") });
      index += 1;
      continue;
    }

    if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)) {
      blocks.push({ type: "hr" });
      index += 1;
      continue;
    }

    if (/^\s{0,3}>\s?/.test(line)) {
      const quote: string[] = [];
      while (index < lines.length && /^\s{0,3}>\s?/.test(lines[index]!)) {
        quote.push(lines[index]!.replace(/^\s{0,3}>\s?/, ""));
        index += 1;
      }
      blocks.push({ type: "quote", children: parseMarkdown(quote.join("\n")) });
      continue;
    }

    if (/^\s*\|.*\|\s*$/.test(line) && index + 1 < lines.length && /^\s*\|?\s*:?-{2,}/.test(lines[index + 1]!)) {
      const header = splitRow(line).map(parseInline);
      index += 2;
      const rows: Inline[][][] = [];
      while (index < lines.length && /^\s*\|.*\|\s*$/.test(lines[index]!)) {
        rows.push(splitRow(lines[index]!).map(parseInline));
        index += 1;
      }
      blocks.push({ type: "table", header, rows });
      continue;
    }

    if (LIST_ITEM.test(line)) {
      const ordered = /^\s*\d+[.)]\s+/.test(line);
      const items: { children: Inline[]; checked: boolean | null }[] = [];
      while (index < lines.length && LIST_ITEM.test(lines[index]!)) {
        const raw = LIST_ITEM.exec(lines[index]!)![3] ?? "";
        const task = TASK_ITEM.exec(raw);
        items.push(task
          ? { children: parseInline(task[2] ?? ""), checked: task[1]!.toLowerCase() === "x" }
          : { children: parseInline(raw), checked: null });
        index += 1;
      }
      blocks.push({ type: "list", ordered, items });
      continue;
    }

    // Consume at least one line so an unsupported marker cannot stall parsing.
    const paragraph: string[] = [];
    do {
      paragraph.push(lines[index]!.replace(/\s{2,}$/, "\u0000"));
      index += 1;
    } while (index < lines.length && lines[index]!.trim() && !isBlockStart(lines[index]!));
    blocks.push({ type: "paragraph", lines: paragraph.join(" ").split("\u0000").map((part) => parseInline(part.trim())) });
  }
  return blocks;
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function inlineToHtml(nodes: Inline[]): string {
  return nodes.map((node) => {
    switch (node.type) {
      case "text": return escapeHtml(node.value);
      case "strong": return `<strong>${inlineToHtml(node.children)}</strong>`;
      case "em": return `<em>${inlineToHtml(node.children)}</em>`;
      case "del": return `<del>${inlineToHtml(node.children)}</del>`;
      case "code": return `<code>${escapeHtml(node.value)}</code>`;
      case "link": return `<a href="${escapeHtml(node.href)}" rel="noreferrer noopener">${inlineToHtml(node.children)}</a>`;
      case "image": return `<img src="${escapeHtml(node.src)}" alt="${escapeHtml(node.alt)}">`;
    }
  }).join("");
}

function blocksToHtml(blocks: Block[]): string {
  return blocks.map((block): string => {
    switch (block.type) {
      case "heading": return `<h${block.level}>${inlineToHtml(block.children)}</h${block.level}>`;
      case "paragraph": return `<p>${block.lines.map((line) => inlineToHtml(line)).join("<br>")}</p>`;
      case "code": return `<pre><code${block.language ? ` class="language-${escapeHtml(block.language)}"` : ""}>${escapeHtml(block.value)}</code></pre>`;
      case "list": {
        const items = block.items.map((item) => item.checked === null
          ? `<li>${inlineToHtml(item.children)}</li>`
          : `<li data-task="true"><input type="checkbox"${item.checked ? " checked" : ""}> ${inlineToHtml(item.children)}</li>`).join("");
        return block.ordered ? `<ol>${items}</ol>` : `<ul${block.items.some((item) => item.checked !== null) ? ' class="md-tasks"' : ""}>${items}</ul>`;
      }
      case "quote": return `<blockquote>${blocksToHtml(block.children)}</blockquote>`;
      case "hr": return "<hr>";
      case "table": return `<table><thead><tr>${block.header.map((cell) => `<th>${inlineToHtml(cell)}</th>`).join("")}</tr></thead><tbody>${block.rows.map((row) => `<tr>${row.map((cell) => `<td>${inlineToHtml(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
    }
  }).join("");
}

/** Escaped HTML for the rich contenteditable surface. */
export function markdownToHtml(source: string): string {
  return blocksToHtml(parseMarkdown(source));
}

function inlineToReact(nodes: Inline[], key: string): ReactNode[] {
  return nodes.map((node, index) => {
    const keyFor = `${key}.${index}`;
    switch (node.type) {
      case "text": return <Fragment key={keyFor}>{node.value}</Fragment>;
      case "strong": return <strong key={keyFor}>{inlineToReact(node.children, keyFor)}</strong>;
      case "em": return <em key={keyFor}>{inlineToReact(node.children, keyFor)}</em>;
      case "del": return <del key={keyFor}>{inlineToReact(node.children, keyFor)}</del>;
      case "code": return <code key={keyFor}>{node.value}</code>;
      case "link": return <a key={keyFor} href={node.href} target="_blank" rel="noreferrer noopener">{inlineToReact(node.children, keyFor)}</a>;
      case "image": return <img key={keyFor} src={node.src} alt={node.alt} loading="lazy" />;
    }
  });
}

function blocksToReact(blocks: Block[], key = "md"): ReactNode[] {
  return blocks.map((block, index) => {
    const keyFor = `${key}.${index}`;
    switch (block.type) {
      case "heading": {
        const Tag = `h${block.level}` as "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
        return <Tag key={keyFor}>{inlineToReact(block.children, keyFor)}</Tag>;
      }
      case "paragraph": return <p key={keyFor}>{block.lines.map((line, lineIndex) => (
        <Fragment key={lineIndex}>{inlineToReact(line, `${keyFor}.${lineIndex}`)}{lineIndex < block.lines.length - 1 ? <br /> : null}</Fragment>
      ))}</p>;
      case "code": return <pre key={keyFor}><code className={block.language ? `language-${block.language}` : undefined}>{block.value}</code></pre>;
      case "list": {
        const ListTag = block.ordered ? "ol" : "ul";
        return <ListTag key={keyFor} className={block.items.some((item) => item.checked !== null) ? "md-tasks" : undefined}>
          {block.items.map((item, itemIndex) => <li key={itemIndex} className={item.checked !== null ? "md-task" : undefined}>
            {item.checked !== null ? <input type="checkbox" checked={item.checked} readOnly tabIndex={-1} aria-label={item.checked ? "Done" : "Open"} /> : null}
            {inlineToReact(item.children, `${keyFor}.${itemIndex}`)}
          </li>)}
        </ListTag>;
      }
      case "quote": return <blockquote key={keyFor}>{blocksToReact(block.children, keyFor)}</blockquote>;
      case "hr": return <hr key={keyFor} />;
      case "table": return <table key={keyFor}>
        <thead><tr>{block.header.map((cell, cellIndex) => <th key={cellIndex}>{inlineToReact(cell, `${keyFor}.h${cellIndex}`)}</th>)}</tr></thead>
        <tbody>{block.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{inlineToReact(cell, `${keyFor}.${rowIndex}.${cellIndex}`)}</td>)}</tr>)}</tbody>
      </table>;
    }
  });
}

/** Renders Markdown source as safe React elements. */
export function MarkdownDocument({ source, className }: { source: string; className?: string | undefined }) {
  return <div className={className ? `markdown ${className}` : "markdown"}>{blocksToReact(parseMarkdown(source))}</div>;
}

/* ------------------------------------------------------------------------- */
/* DOM → Markdown (save path)                                                */
/* ------------------------------------------------------------------------- */

function inlineFromDom(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return (node.textContent ?? "").replace(/\u00a0/g, " ");
  if (node.nodeType !== Node.ELEMENT_NODE) return "";
  const element = node as HTMLElement;
  const tag = element.tagName.toLowerCase();
  const inner = () => Array.from(element.childNodes).map(inlineFromDom).join("");
  switch (tag) {
    case "strong":
    case "b": return `**${inner()}**`;
    case "em":
    case "i": return `_${inner()}_`;
    case "del":
    case "s":
    case "strike": return `~~${inner()}~~`;
    case "code": return `\`${inner()}\``;
    case "a": {
      const href = safeUrl(element.getAttribute("href") ?? "");
      const text = inner();
      return href ? `[${text}](${href})` : text;
    }
    case "img": {
      const src = safeUrl(element.getAttribute("src") ?? "");
      return src ? `![${element.getAttribute("alt") ?? ""}](${src})` : "";
    }
    case "br": return "  \n";
    case "input": return "";
    case "span": {
      const text = inner();
      const style = element.style;
      const bold = /bold|[6-9]00/.test(style.fontWeight);
      const italic = style.fontStyle === "italic";
      const strike = style.textDecoration.includes("line-through");
      let value = text;
      if (italic) value = `_${value}_`;
      if (bold) value = `**${value}**`;
      if (strike) value = `~~${value}~~`;
      return value;
    }
    default: return inner();
  }
}

function listFromDom(element: HTMLElement, depth: number): string {
  const ordered = element.tagName.toLowerCase() === "ol";
  const lines: string[] = [];
  let index = 1;
  for (const child of Array.from(element.children)) {
    if (child.tagName.toLowerCase() !== "li") continue;
    const item = child as HTMLElement;
    const checkbox = item.querySelector<HTMLInputElement>('input[type="checkbox"]');
    const nested = Array.from(item.children).filter((entry) => ["ul", "ol"].includes(entry.tagName.toLowerCase())) as HTMLElement[];
    const content = Array.from(item.childNodes)
      .filter((entry) => !(entry.nodeType === Node.ELEMENT_NODE && ["ul", "ol"].includes((entry as Element).tagName.toLowerCase())))
      .map(inlineFromDom)
      .join("")
      .trim();
    const marker = checkbox ? `- [${checkbox.checked ? "x" : " "}]` : ordered ? `${index}.` : "-";
    lines.push(`${"  ".repeat(depth)}${marker} ${content}`.trimEnd());
    index += 1;
    for (const nestedList of nested) lines.push(listFromDom(nestedList, depth + 1));
  }
  return lines.join("\n");
}

function tableFromDom(element: HTMLElement): string {
  const rows = Array.from(element.querySelectorAll("tr"));
  if (!rows.length) return "";
  const cells = (row: Element) => Array.from(row.querySelectorAll("th,td")).map((cell) => inlineFromDom(cell).replace(/\\/g, "\\\\").replace(/\|/g, "\\|").trim());
  const header = cells(rows[0]!);
  const lines = [
    `| ${header.join(" | ")} |`,
    `| ${header.map(() => "---").join(" | ")} |`,
    ...rows.slice(1).map((row) => `| ${cells(row).join(" | ")} |`)
  ];
  return lines.join("\n");
}

function blockFromDom(node: Node): string | null {
  if (node.nodeType === Node.TEXT_NODE) return (node.textContent ?? "").trim() || null;
  if (node.nodeType !== Node.ELEMENT_NODE) return null;
  const element = node as HTMLElement;
  const tag = element.tagName.toLowerCase();
  if (element.dataset.task === "true" || element.classList.contains("md-task-line")) {
    const checkbox = element.querySelector<HTMLInputElement>('input[type="checkbox"]');
    return `- [${checkbox?.checked ? "x" : " "}] ${inlineFromDom(element).replace(/^\s+/, "")}`;
  }
  switch (tag) {
    case "h1": case "h2": case "h3": case "h4": case "h5": case "h6":
      return `${"#".repeat(Number(tag.slice(1)))} ${inlineFromDom(element).trim()}`;
    case "p": return inlineFromDom(element).trim();
    case "div": {
      if (element.querySelector("ul,ol,table,blockquote,pre,h1,h2,h3,h4,h5,h6")) {
        return Array.from(element.childNodes).map(blockFromDom).filter((value): value is string => value !== null).join("\n\n");
      }
      return inlineFromDom(element).trim();
    }
    case "ul": case "ol": return listFromDom(element, 0);
    case "blockquote": {
      const inner = Array.from(element.childNodes).map(blockFromDom).filter((value): value is string => value !== null).join("\n\n");
      return inner.split("\n").map((line) => `> ${line}`.trimEnd()).join("\n");
    }
    case "pre": {
      const code = element.querySelector("code");
      const language = /language-([\w-]+)/.exec(code?.className ?? "")?.[1] ?? "";
      return `\`\`\`${language}\n${code?.textContent ?? element.textContent ?? ""}\n\`\`\``;
    }
    case "hr": return "---";
    case "table": return tableFromDom(element);
    case "br": return null;
    default: return inlineFromDom(element).trim() || null;
  }
}

/** Serializes the rich editor DOM back to Markdown. */
export function htmlToMarkdown(root: HTMLElement): string {
  const blocks = Array.from(root.childNodes)
    .map(blockFromDom)
    .filter((value): value is string => value !== null && value.trim().length > 0);
  return blocks.join("\n\n").replace(/\n{3,}/g, "\n\n").trim();
}
