/**
 * Theme icon packs.
 *
 * A theme may ship its own artwork for the shell's system icons. Icon packs are
 * plain data — never HTML or SVG source — so they render through React under
 * SSR and the strict CSP, and an allowlist keeps them presentational: no
 * scripts, links, images, filters or event handlers can reach the page.
 */
export const ICON_NAMES = [
  "home",
  "phone",
  "store",
  "grid",
  "pulse",
  "settings",
  "search",
  "sliders",
  "moon",
  "spark",
  "bolt",
  "arrow",
  "arrow-left",
  "shield",
  "user",
  "desktop",
  "info",
  "warning",
  "document",
  "external-link",
  "terminal",
  "window-minimize",
  "window-maximize",
  "window-restore",
  "window-close",
  "edit",
  "play",
  "stop",
  "refresh",
  "check",
  "window-stack",
  "window-fullscreen",
  "window-exit-fullscreen"
] as const;

export type IconName = (typeof ICON_NAMES)[number];

const ICON_SET = new Set<string>(ICON_NAMES);

export type ThemeIconShapeTag =
  | "path"
  | "circle"
  | "rect"
  | "line"
  | "polyline"
  | "polygon"
  | "ellipse"
  | "g";

export interface ThemeIconShape {
  tag: ThemeIconShapeTag;
  /** Sanitized, camelCased SVG presentation attributes. */
  attrs: Readonly<Record<string, string | number>>;
  children?: readonly ThemeIconShape[];
}

export interface ThemeIcon {
  /** Four-number viewBox; defaults to the shell's `0 0 24 24`. */
  viewBox: string;
  /** Filled artwork instead of the shell's stroke rendering. */
  filled: boolean;
  shapes: readonly ThemeIconShape[];
}

export type IconPack = Partial<Record<IconName, ThemeIcon>>;

/** Authoring shape for `defineTheme({ icons })`; `viewBox`/`filled` are optional. */
export interface ThemeIconInput {
  viewBox?: string | undefined;
  filled?: boolean | undefined;
  shapes: readonly ThemeIconShape[];
}
export type IconPackInput = Partial<Record<IconName, ThemeIconInput>>;

export interface IconPackReport {
  icons: IconPack;
  errors: readonly string[];
  warnings: readonly string[];
}

const MAX_ICONS = 48;
const MAX_SHAPES = 32;
const MAX_DEPTH = 3;
const MAX_PATH = 4096;
const MAX_VALUE = 512;

const COMMON_ATTRS = [
  "fill",
  "stroke",
  "strokeWidth",
  "strokeLinecap",
  "strokeLinejoin",
  "fillRule",
  "clipRule",
  "opacity",
  "transform"
] as const;

const TAG_ATTRS: Record<ThemeIconShapeTag, readonly string[]> = {
  path: ["d"],
  circle: ["cx", "cy", "r"],
  ellipse: ["cx", "cy", "rx", "ry"],
  rect: ["x", "y", "width", "height", "rx", "ry"],
  line: ["x1", "y1", "x2", "y2"],
  polyline: ["points"],
  polygon: ["points"],
  g: []
};

const SHAPE_TAGS = new Set<string>(Object.keys(TAG_ATTRS));
const NUMBER_ATTRS = new Set([
  "cx",
  "cy",
  "r",
  "rx",
  "ry",
  "x",
  "y",
  "width",
  "height",
  "x1",
  "y1",
  "x2",
  "y2",
  "opacity",
  "strokeWidth"
]);
const PATH_DATA = /^[MmZzLlHhVvCcSsQqTtAa0-9eE.,\s+-]+$/;
const TRANSFORM = /^[a-zA-Z0-9\s.,()%+-]+$/;
const PAINT =
  /^(?:none|currentColor|transparent|#[0-9a-f]{3,8}|[a-z]+|(?:rgb|rgba|hsl|hsla)\([0-9.,%\s/]+\))$/i;
const LINE_CAP = new Set(["butt", "round", "square"]);
const LINE_JOIN = new Set(["arcs", "bevel", "miter", "miter-clip", "round"]);
const FILL_RULE = new Set(["nonzero", "evenodd"]);
const VIEW_BOX = /^-?\d+(?:\.\d+)?(?: -?\d+(?:\.\d+)?){3}$/;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readValue(name: string, value: unknown): string | number | null {
  if (NUMBER_ATTRS.has(name)) {
    const parsed = typeof value === "number" ? value : typeof value === "string" ? Number(value) : Number.NaN;
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (typeof value !== "string" || value.length > MAX_VALUE) return null;
  if (name === "d") return PATH_DATA.test(value) && value.length <= MAX_PATH ? value : null;
  if (name === "points") return PATH_DATA.test(value) ? value : null;
  if (name === "transform") return TRANSFORM.test(value) ? value : null;
  if (name === "strokeLinecap") return LINE_CAP.has(value) ? value : null;
  if (name === "strokeLinejoin") return LINE_JOIN.has(value) ? value : null;
  if (name === "fillRule" || name === "clipRule") return FILL_RULE.has(value) ? value : null;
  if (name === "fill" || name === "stroke") return PAINT.test(value) ? value : null;
  return null;
}

function sanitizeShape(input: unknown, depth: number): ThemeIconShape | null {
  if (depth > MAX_DEPTH || !isRecord(input)) return null;
  if (typeof input.tag !== "string" || !SHAPE_TAGS.has(input.tag)) return null;
  const tag = input.tag as ThemeIconShapeTag;
  const allowed = new Set<string>([...COMMON_ATTRS, ...TAG_ATTRS[tag]]);
  const attrs: Record<string, string | number> = {};
  if (input.attrs !== undefined) {
    if (!isRecord(input.attrs)) return null;
    for (const [rawName, rawValue] of Object.entries(input.attrs)) {
      const name = rawName.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase());
      if (!allowed.has(name)) return null;
      const value = readValue(name, rawValue);
      if (value === null) return null;
      attrs[name] = value;
    }
  }
  const shape: ThemeIconShape = { tag, attrs };
  if (input.children !== undefined) {
    if (!Array.isArray(input.children) || depth >= MAX_DEPTH) return null;
    const children = input.children
      .slice(0, MAX_SHAPES)
      .map((child) => sanitizeShape(child, depth + 1))
      .filter((child): child is ThemeIconShape => child !== null);
    if (children.length !== input.children.length) return null;
    if (children.length) shape.children = children;
  }
  return shape;
}

/**
 * Validates an icon pack. Unsafe markup (unknown tags/attributes, links,
 * external references, oversized paths) drops the affected icon and is
 * reported as an error; an unknown icon name is only a warning.
 */
export function readIconPack(value: unknown): IconPackReport {
  if (value === undefined || value === null) return { icons: {}, errors: [], warnings: [] };
  if (!isRecord(value)) {
    return { icons: {}, errors: ["Icon pack must be an object keyed by icon name."], warnings: [] };
  }
  const icons: IconPack = {};
  const errors: string[] = [];
  const warnings: string[] = [];
  for (const [name, raw] of Object.entries(value).slice(0, MAX_ICONS)) {
    if (!ICON_SET.has(name)) {
      warnings.push(`Unknown icon "${name}" is ignored.`);
      continue;
    }
    if (!isRecord(raw)) {
      errors.push(`Icon "${name}" must be an object.`);
      continue;
    }
    const viewBox = raw.viewBox === undefined ? "0 0 24 24" : raw.viewBox;
    if (typeof viewBox !== "string" || !VIEW_BOX.test(viewBox)) {
      errors.push(`Icon "${name}" has an invalid viewBox.`);
      continue;
    }
    if (!Array.isArray(raw.shapes) || raw.shapes.length === 0 || raw.shapes.length > MAX_SHAPES) {
      errors.push(`Icon "${name}" needs 1-${MAX_SHAPES} shapes.`);
      continue;
    }
    const shapes = raw.shapes
      .map((shape) => sanitizeShape(shape, 0))
      .filter((shape): shape is ThemeIconShape => shape !== null);
    if (shapes.length !== raw.shapes.length) {
      errors.push(`Icon "${name}" contains unsupported SVG markup.`);
      continue;
    }
    icons[name as IconName] = { viewBox, filled: raw.filled === true, shapes };
  }
  return { icons, errors, warnings };
}

/** Returns only the safe icons; the safety engine reports what was dropped. */
export function sanitizeIconPack(value: unknown): IconPack {
  return readIconPack(value).icons;
}
