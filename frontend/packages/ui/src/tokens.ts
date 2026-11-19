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
  // Semantic feedback roles. Components use these instead of local red/green
  // values, and always pair them with a label or icon.
  "color.success": "#1e6b46",
  "color.success.surface": "#e8f5ee",
  "color.warning": "#7a4b00",
  "color.warning.surface": "#fff3d6",
  "color.danger": "#a3231c",
  "color.danger.surface": "#fde9e7",
  "color.info": "#1f5c91",
  "color.info.surface": "#e8f2fc",
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
  "shape.radius.window": "23px",
  "shape.radius.dock": "28px",
  "shape.radius.icon": "18px",
  "shape.icon.size": "52px",
  "shape.icon.size.large": "68px",
  "motion.duration.fast": "120ms",
  "motion.duration": "180ms",
  "motion.easing.spring": "cubic-bezier(.32, .72, 0, 1)",
  // OS interaction contract: a readable type floor, a prose measure and hit
  // areas that remain usable with a thumb.
  "typography.weight.regular": "400",
  "typography.weight.medium": "600",
  "typography.weight.strong": "750",
  "typography.size.floor": "12px",
  "typography.size.metadata": "12px",
  "typography.measure": "68ch",
  "space.1": "4px",
  "space.2": "8px",
  "space.3": "12px",
  "space.4": "16px",
  "space.5": "24px",
  "space.6": "28px",
  "target.min": "44px",
  "target.large": "48px",
  "target.gap": "8px",
  "layout.dock.offset": "10px",
  "icon.gradient": "linear-gradient(150deg, #4f8f78, #28694c)",
  "texture.wallpaper": "default",
  "typography.family": FONT_STACKS.modern,
  "typography.display": FONT_STACKS.spaceGrotesk,
  "typography.scale": "1",
  // Material of the rumahl/ui app input components: "glass" (recessed glass) or
  // "solid" (flat). Independent of the shell material; driven by a theme
  // parameter so a theme (and the user) can choose it.
  "app.inputs": "glass",
  // Iris keeps its original inset glass; the quieter group recipe is opt-in.
  "iris.controls": "glass"
} as const satisfies Record<string, string>;

export type TokenId = keyof typeof defaultTokens;
export type Tokens = Readonly<Record<string, string>> & Readonly<Record<TokenId, string>>;

/**
 * Neutral functional fallbacks for themes that build on Ground.
 *
 * Ground is not a theme. These values keep a ground-based theme usable without
 * borrowing Iris branding: system fonts, a neutral blue action colour, square
 * geometry, no glass and no photographic wallpaper. A theme may replace any of
 * them through its own tokens.
 */
export const groundTokens = {
  ...defaultTokens,
  "color.accent": "#1a73e8",
  "color.accent.strong": "#4b8bf0",
  "color.on.accent": "#ffffff",
  "color.surface": "#f2f2f2",
  "color.surface.strong": "#ffffff",
  "color.outline": "#767676",
  "color.shadow": "rgba(0, 0, 0, 0.28)",
  "color.text.primary": "#000000",
  "color.text.muted": "#5f6368",
  "color.canvas.background": "#ffffff",
  "color.panel.background": "#ffffff",
  "color.window.titlebar.background": "#f2f2f2",
  "color.window.titlebar.foreground": "#000000",
  "color.on.wallpaper": "#000000",
  "color.wallpaper.tint": "#00000000",
  "material.blur": "0px",
  "material.saturation": "1",
  "material.opacity": "1",
  "material.morphism": "0",
  "shape.radius.window": "0px",
  "shape.radius.dock": "0px",
  "shape.radius.icon": "0px",
  "typography.family": FONT_STACKS.system,
  "typography.display": FONT_STACKS.system,
  "typography.size.floor": "12px",
  "typography.size.metadata": "12px",
  "icon.gradient": "linear-gradient(150deg, #6b7280, #374151)",
  "texture.wallpaper": "linear-gradient(#ffffff, #ffffff)",
  "app.inputs": "solid"
} as const satisfies Tokens;

/** Resolves the neutral structural fallbacks for a theme base. */
export function tokensForBase(base: "iris" | "ground" = "iris"): Tokens {
  return base === "ground" ? groundTokens : defaultTokens;
}

/**
 * Merges token layers (base defaults, theme package, device snapshot). Absent
 * values are ignored so a package cannot blank a token with `undefined`.
 */
export function mergeTokens(...layers: readonly (Partial<Tokens> | undefined)[]): Tokens {
  const merged: Record<string, string> = {};
  for (const layer of layers) {
    if (!layer) continue;
    for (const [id, value] of Object.entries(layer)) {
      if (typeof value === "string") merged[id] = value;
    }
  }
  return merged as Tokens;
}

export const TOKEN_IDS = Object.keys(defaultTokens) as readonly TokenId[];

/** CSS custom property that backs a token id (themes may define their own). */
export function tokenVariable(id: string): string {
  return `--rumahl-ui-${id.replaceAll(".", "-")}`;
}

/**
 * Applies every token as a CSS custom property on `root` (CSP-safe CSSOM).
 * Theme packages may ship tokens beyond the built-in contract; they are applied
 * exactly like the known ones.
 */
export function applyTokens(root: HTMLElement, tokens: Readonly<Record<string, string>>): void {
  for (const [id, value] of Object.entries(tokens)) root.style.setProperty(tokenVariable(id), value);
}

/** Removes the given tokens (or every built-in one) from `root`. */
export function clearTokens(root: HTMLElement, tokens?: Readonly<Record<string, string>>): void {
  for (const id of tokens ? Object.keys(tokens) : TOKEN_IDS) root.style.removeProperty(tokenVariable(id));
}
