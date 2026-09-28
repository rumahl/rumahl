/**
 * Default rumahl OS design tokens.
 *
 * A theme provides values for every token id; the runtime projects them onto
 * the document root as `--rumahl-ui-*` custom properties. The values below are
 * the rumahl brand defaults (green accent, glass materials, macOS-like shapes).
 * They intentionally use plain CSS values so a theme can change colours,
 * transparency, radii, motion and textures without touching component code.
 */
export const defaultTokens = {
  "color.accent": "#28694c",
  "color.accent-strong": "#4f8f78",
  "color.surface": "rgba(248, 250, 247, 0.84)",
  "color.surface-strong": "#f8faf7",
  "color.outline": "rgba(255, 255, 255, 0.44)",
  "color.on-wallpaper": "#ffffff",
  "color.on-accent": "#ffffff",
  "color.shadow": "rgba(0, 0, 0, 0.27)",
  "material.blur": "34px",
  "material.saturation": "1.6",
  "shape.radius-window": "12px",
  "shape.radius-dock": "22px",
  "shape.radius-icon": "15px",
  "shape.icon-size": "50px",
  "shape.icon-size-large": "64px",
  "motion.duration-fast": "120ms",
  "motion.duration": "150ms",
  "motion.easing-spring": "cubic-bezier(.2, .9, .3, 1.25)",
  "layout.dock-offset": "12px",
  "icon.gradient": "linear-gradient(140deg, #3f9e74, #28694c)"
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
