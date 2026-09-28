import { RUMAHL_COLORS, themeColorValues } from "./color";

/**
 * Default rumahl OS design tokens.
 *
 * Colour roles are computed by the native rumahl colour system from a single
 * seed (see `@rumahl/ui/color`); structural roles (materials, shapes, motion,
 * typography) are authored here. A theme projects every token onto the document
 * root as `--rumahl-ui-*` custom properties. Token ids match the authoritative
 * Rust contract exactly (dots).
 */
const COLORS = themeColorValues(RUMAHL_COLORS.base);

export const defaultTokens = {
  "color.accent": COLORS.accent,
  "color.accent.strong": COLORS.accentStrong,
  "color.surface": COLORS.surface,
  "color.surface.strong": COLORS.surfaceStrong,
  "color.outline": "rgba(255, 255, 255, 0.44)",
  "color.on.wallpaper": "#ffffff",
  "color.on.accent": COLORS.onAccent,
  "color.shadow": "rgba(0, 0, 0, 0.27)",
  "color.text.primary": "#18201c",
  "color.text.muted": "#657068",
  "color.canvas.background": "#e8ece7",
  "color.panel.background": "#f8faf7",
  "color.window.titlebar.background": "#f2f5f1",
  "color.window.titlebar.foreground": "#18201c",
  "material.blur": "34px",
  "material.saturation": "1.6",
  "material.opacity": "0.84",
  "shape.radius.window": "12px",
  "shape.radius.dock": "22px",
  "shape.radius.icon": "15px",
  "shape.icon.size": "50px",
  "shape.icon.size.large": "64px",
  "motion.duration.fast": "120ms",
  "motion.duration": "150ms",
  "motion.easing.spring": "cubic-bezier(.2, .9, .3, 1.25)",
  "layout.dock.offset": "12px",
  "icon.gradient": "linear-gradient(140deg, #3f9e74, #28694c)",
  "texture.wallpaper": "default",
  "typography.family": "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif",
  "typography.scale": "1"
} as const satisfies Record<string, string>;

export type TokenId = keyof typeof defaultTokens;
export type Tokens = Readonly<Record<TokenId, string>>;

export const TOKEN_IDS = Object.keys(defaultTokens) as readonly TokenId[];

/** CSS custom property that backs a token id. */
export function tokenVariable(id: TokenId): string {
  return `--rumahl-ui-${id.replaceAll(".", "-")}`;
}

/** Applies every token as a CSS custom property on `root` (CSP-safe CSSOM). */
export function applyTokens(root: HTMLElement, tokens: Tokens): void {
  for (const id of TOKEN_IDS) root.style.setProperty(tokenVariable(id), tokens[id]);
}

/** Removes every managed token from `root`. */
export function clearTokens(root: HTMLElement): void {
  for (const id of TOKEN_IDS) root.style.removeProperty(tokenVariable(id));
}
