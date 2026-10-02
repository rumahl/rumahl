import { createElement, Fragment, type ReactNode } from "react";

/**
 * Safe slot templates. A theme may ship markup for shell regions as plain data
 * (a whitelisted element tree). It is validated here and rendered by the shell —
 * no arbitrary HTML/JS reaches the page, so it is safe under SSR and the strict
 * CSP. This is the theme-code surface: themes decide what a region (dock,
 * menubar, …) looks like, while the shell owns behaviour.
 */
export const SLOT_NAMES = ["dock", "menubar", "window", "desktop", "launcher"] as const;
export type SlotName = (typeof SLOT_NAMES)[number];

export interface SlotText { text: string }
export interface SlotRepeat { repeat: string; as: string; children: SlotNode[] }
export interface SlotComponent {
  /** A shell-provided component (whitelisted; never arbitrary code). */
  component: string;
  class?: string;
  attrs?: Record<string, string>;
  children?: SlotNode[];
}
export interface SlotElement {
  tag: string;
  class?: string;
  attrs?: Record<string, string>;
  /** Named shell action (see SlotActions); never an inline handler. */
  action?: string;
  arg?: string;
  children?: SlotNode[];
}
export type SlotNode = SlotElement | SlotText | SlotRepeat | SlotComponent;
export type SlotTemplate = readonly SlotNode[];

/** Elements a theme may use. Deliberately small and presentational. */
const TAGS = new Set([
  "div", "span", "nav", "header", "footer", "section", "aside", "main", "article",
  "button", "a", "ul", "ol", "li", "p", "strong", "small", "b", "i", "em", "mark",
  "abbr", "code", "pre", "blockquote", "q", "cite", "dl", "dt", "dd", "hr", "br",
  "img", "picture", "source", "video", "audio", "time",
  "table", "thead", "tbody", "tfoot", "tr", "th", "td",
  "form", "label", "input", "select", "option", "textarea", "fieldset", "legend",
  "details", "summary", "dialog", "figure", "figcaption"
]);
const ATTRS = new Set([
  "id", "title", "role", "href", "type", "alt", "src", "width", "height", "disabled",
  "hidden", "tabindex", "datetime", "for", "name", "value", "placeholder", "checked",
  "selected", "open", "controls", "autoplay", "muted", "loop", "playsinline", "poster",
  "colspan", "rowspan", "download", "target", "rel", "spellcheck", "autocomplete"
]);
const SAFE_URL = /^(?:https?:\/\/|\/|#|data:image\/|data:video\/|data:audio\/)/;
const INPUT_TYPES = new Set(["text", "search", "number", "checkbox", "radio", "range", "color", "date", "time", "email", "url", "tel", "password"]);

function sanitizeAttrs(input: unknown): Record<string, string> | undefined {
  if (!input || typeof input !== "object") return undefined;
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(input as Record<string, unknown>)) {
    if (typeof value !== "string" || value.length > 512) continue;
    const lower = key.toLowerCase();
    if (lower.startsWith("on") || lower === "style" || lower === "srcset") continue;
    const ok = ATTRS.has(lower) || lower.startsWith("aria-") || lower.startsWith("data-");
    if (!ok) continue;
    if ((lower === "href" || lower === "src" || lower === "poster") && !SAFE_URL.test(value)) continue;
    out[key] = value;
  }
  return Object.keys(out).length ? out : undefined;
}

function sanitizeNode(input: unknown, depth: number): SlotNode | null {
  if (depth > 12 || !input || typeof input !== "object") return null;
  const record = input as Record<string, unknown>;
  if (typeof record.text === "string") return { text: record.text.slice(0, 512) };
  if (typeof record.repeat === "string") {
    if (typeof record.as !== "string" || !Array.isArray(record.children)) return null;
    const children = record.children.map((child) => sanitizeNode(child, depth + 1)).filter((node): node is SlotNode => node !== null);
    return { repeat: record.repeat, as: record.as, children };
  }
  if (typeof record.component === "string") {
    if (!/^[a-z][a-z0-9-]*$/.test(record.component)) return null;
    const component: SlotComponent = { component: record.component };
    if (typeof record.class === "string" && record.class.length <= 512) component.class = record.class;
    const componentAttrs = sanitizeAttrs(record.attrs);
    if (componentAttrs) component.attrs = componentAttrs;
    if (Array.isArray(record.children)) {
      const children = record.children.map((child) => sanitizeNode(child, depth + 1)).filter((entry): entry is SlotNode => entry !== null);
      if (children.length) component.children = children;
    }
    return component;
  }
  if (typeof record.tag !== "string" || !TAGS.has(record.tag)) return null;
  const node: SlotElement = { tag: record.tag };
  if (typeof record.class === "string" && record.class.length <= 512) node.class = record.class;
  const attrs = sanitizeAttrs(record.attrs);
  if (attrs && node.tag === "input" && attrs.type && !INPUT_TYPES.has(attrs.type)) delete attrs.type;
  if (attrs) node.attrs = attrs;
  if (typeof record.action === "string" && /^[a-z][a-z0-9-]*$/.test(record.action)) node.action = record.action;
  if (typeof record.arg === "string" && record.arg.length <= 512) node.arg = record.arg;
  if (Array.isArray(record.children)) {
    const children = record.children.map((child) => sanitizeNode(child, depth + 1)).filter((entry): entry is SlotNode => entry !== null);
    if (children.length) node.children = children;
  }
  return node;
}

export function sanitizeSlot(value: unknown): SlotTemplate {
  if (!Array.isArray(value)) return [];
  return value.map((node) => sanitizeNode(node, 0)).filter((node): node is SlotNode => node !== null);
}

export function sanitizeSlots(value: unknown): Partial<Record<SlotName, SlotTemplate>> {
  if (!value || typeof value !== "object") return {};
  const out: Partial<Record<SlotName, SlotTemplate>> = {};
  for (const name of SLOT_NAMES) {
    const template = (value as Record<string, unknown>)[name];
    if (template === undefined) continue;
    out[name] = sanitizeSlot(template);
  }
  return out;
}

export interface SlotComponentProps {
  className?: string | undefined;
  children?: ReactNode;
  [key: string]: unknown;
}
export interface SlotBindings {
  /** Data available to templates (e.g. `apps`, `windows`, `mode`). */
  data: Record<string, unknown>;
  /** Whitelisted shell actions a template may trigger. */
  actions: Record<string, (arg?: string) => void>;
  /** Components the shell exposes to templates (icons, clock, …). */
  components?: Record<string, (props: SlotComponentProps) => ReactNode>;
}

function resolve(path: string, scopes: Record<string, unknown>[]): unknown {
  const parts = path.trim().split(".");
  for (const scope of scopes) {
    let value: unknown = scope;
    for (const part of parts) {
      if (!value || typeof value !== "object") { value = undefined; break; }
      value = (value as Record<string, unknown>)[part];
    }
    if (value !== undefined) return value;
  }
  return undefined;
}

/** Replaces `{{path.to.value}}` with a bound value (never evaluated as code). */
function interpolate(text: string, scopes: Record<string, unknown>[]): string {
  return text.replace(/\{\{\s*([\w.]+)\s*\}\}/g, (_, path: string) => {
    const value = resolve(path, scopes);
    return value === undefined || value === null ? "" : String(value);
  });
}

function renderNodes(nodes: SlotTemplate, bindings: SlotBindings, scopes: Record<string, unknown>[], keyPrefix: string): ReactNode[] {
  return nodes.map((node, index) => {
    const key = `${keyPrefix}${index}`;
    if ("text" in node) return interpolate(node.text, scopes);
    if ("repeat" in node) {
      const list = resolve(node.repeat, scopes);
      if (!Array.isArray(list)) return null;
      return list.map((entry, entryIndex) => (
        <Fragment key={`${key}.${entryIndex}`}>
          {renderNodes(node.children, bindings, [typeof entry === "object" && entry ? entry as Record<string, unknown> : { [node.as]: entry }, ...scopes], `${key}.${entryIndex}.`)}
        </Fragment>
      ));
    }
    if ("component" in node) {
      const render = bindings.components?.[node.component];
      if (!render) return null;
      const children = node.children ? renderNodes(node.children, bindings, scopes, `${key}.`) : undefined;
      return <Fragment key={key}>{render({ ...(node.attrs ?? {}), className: node.class, children })}</Fragment>;
    }
    const props: Record<string, unknown> = { key };
    if (node.class) props.className = node.class;
    if (node.attrs) Object.assign(props, node.attrs);
    const action = node.action ? bindings.actions[node.action] : undefined;
    if (action) props.onClick = () => action(node.arg ? interpolate(node.arg, scopes) : undefined);
    const children = node.children ? renderNodes(node.children, bindings, scopes, `${key}.`) : [];
    return createElement(node.tag, props, children.length ? children : undefined);
  });
}


/** Renders a validated slot template to React nodes. */
export function renderSlot(template: SlotTemplate, bindings: SlotBindings): ReactNode {
  return renderNodes(template, bindings, [bindings.data], "");
}
