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

/**
 * System font stacks offered as typography variants. No webfonts are bundled
 * (the shell CSP blocks `font-src`), so every variant resolves against fonts
 * already present on the device.
 */
export const FONT_STACKS = {
  modern: '"Inter", "SF Pro Text", system-ui, -apple-system, "Segoe UI Variable Text", "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
  system: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Noto Sans", sans-serif',
  neutral: '"Helvetica Neue", Helvetica, Arial, system-ui, sans-serif',
  rounded: '"SF Pro Rounded", ui-rounded, "Nunito", "Varela Round", system-ui, sans-serif',
  serif: '"New York", "Iowan Old Style", Georgia, "Times New Roman", Times, serif',
  mono: 'ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace',
  // Bundled display/statement faces (local webfonts; used for the launcher greeting).
  spaceGrotesk: '"Space Grotesk Variable", system-ui, sans-serif',
  clashDisplay: '"Clash Display Variable", "Space Grotesk Variable", system-ui, sans-serif',
  bricolageGrotesque: '"Bricolage Grotesque Variable", system-ui, sans-serif',
  instrumentSerif: '"Instrument Serif", "New York", Georgia, serif',
  // Open-source Google-Fonts-style faces (bundled locally via @fontsource).
  outfit: '"Outfit Variable", system-ui, sans-serif',
  sora: '"Sora Variable", system-ui, sans-serif',
  syne: '"Syne Variable", system-ui, sans-serif',
  unbounded: '"Unbounded Variable", system-ui, sans-serif',
  fraunces: '"Fraunces Variable", "New York", Georgia, serif',
  playfairDisplay: '"Playfair Display Variable", Georgia, serif',
  dmSerifDisplay: '"DM Serif Display", Georgia, serif',
  gabarito: '"Gabarito Variable", system-ui, sans-serif',
  manrope: '"Manrope Variable", system-ui, sans-serif',
  plusJakartaSans: '"Plus Jakarta Sans Variable", system-ui, sans-serif'
} as const;

export type FontVariant = keyof typeof FONT_STACKS;

export const defaultTokens = {
  "color.accent": COLORS.accent,
  "color.accent.strong": COLORS.accentStrong,
  // Neutral system surfaces; only the accent carries the brand hue.
  "color.surface": "#ffffffd9",
  "color.surface.strong": "#ffffff",
  "color.outline": "rgba(0, 0, 0, 0.12)",
  "color.on.wallpaper": "#ffffff",
  "color.on.accent": COLORS.onAccent,
  "color.shadow": "rgba(0, 0, 0, 0.16)",
  "color.wallpaper.tint": "#00000000",
  "color.text.primary": "#1d1d1f",
  "color.text.muted": "#6e6e73",
  "color.canvas.background": "#c9c9ce",
  "color.panel.background": "#f5f5f7",
  "color.window.titlebar.background": "#f5f5f7",
  "color.window.titlebar.foreground": "#1d1d1f",
  "material.blur": "60px",
  "material.saturation": "1.8",
  "material.opacity": "0.7",
  "material.morphism": "1",
  "shape.radius.window": "12px",
  "shape.radius.dock": "24px",
  "shape.radius.icon": "15px",
  "shape.icon.size": "52px",
  "shape.icon.size.large": "68px",
  "motion.duration.fast": "120ms",
  "motion.duration": "180ms",
  "motion.easing.spring": "cubic-bezier(.32, .72, 0, 1)",
  "layout.dock.offset": "10px",
  "icon.gradient": "linear-gradient(150deg, #4f8f78, #28694c)",
  "texture.wallpaper": "default",
  "typography.family": FONT_STACKS.modern,
  "typography.display": FONT_STACKS.spaceGrotesk,
  "typography.scale": "1",
  // Material of the rumahl/ui app input components: "glass" (recessed glass) or
  // "solid" (flat). Independent of the shell material; driven by a theme
  // parameter so a theme (and the user) can choose it.
  "app.inputs": "glass"
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
