import { bestContrast, findShade, generateColorShades, hexToRgb, themeColorValues, withAlpha, type PaletteOptions } from "./color";
import type { Tokens } from "./tokens";

function channel(value: number): string {
  const hex = Math.round(Math.min(Math.max(value, 0), 255)).toString(16);
  return hex.length === 1 ? `0${hex}` : hex;
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${channel(r)}${channel(g)}${channel(b)}`;
}

/** Alpha-composites `foreground` over `background`. */
export function composite(background: string, foreground: string, alpha: number): string {
  const [br, bg, bb] = hexToRgb(background);
  const [fr, fg, fb] = hexToRgb(foreground);
  const a = Math.min(Math.max(alpha, 0), 1);
  return rgbToHex(br + (fr - br) * a, bg + (fg - bg) * a, bb + (fb - bb) * a);
}

/**
 * Picks readable text colours for a surface at a given opacity. As the surface
 * becomes more transparent the effective colour blends towards the canvas, so
 * the returned text contrast adapts automatically.
 */
export function surfaceContrast(surface: string, canvas: string, opacity: number): { textPrimary: string; textMuted: string } {
  const effective = composite(canvas, surface, Number.isFinite(opacity) ? opacity : 1);
  return {
    textPrimary: bestContrast(["#101418", "#ffffff"], effective),
    textMuted: bestContrast(["#5b6668", "#c9d4d8"], effective)
  };
}

/**
 * Projects the rumahl colour engine onto the theme colour roles. Light mode
 * keeps neutral surfaces so the OS never inherits a strong tint
 * from the brand colour. Dark mode is derived from the **native rumahl colour
 * engine** (seed → dark shades) with distinct canvas, panel and frosted surface depths.
 */
export function paletteTokens(seed: string, options: PaletteOptions = {}): Partial<Tokens> {
  const colors = themeColorValues(seed, options);
  const accent = {
    "color.accent": colors.accent,
    "color.accent.strong": colors.accentStrong,
    "color.on.accent": colors.onAccent,
    "icon.gradient": `linear-gradient(150deg, ${colors.accentStrong}, ${colors.accent})`
  };
  const shade = (table: ReturnType<typeof generateColorShades>["primary"]["dark"], level: number, fallback: string) =>
    findShade(table, level)?.hex ?? fallback;

  if (options.mode === "dark") {
    const dark = generateColorShades(seed, options).primary.dark;
    const canvas = composite("#1b2028", shade(dark, 700, "#17221e"), 0.18);
    const panel = composite("#2b313c", shade(dark, 650, "#22342b"), 0.18);
    const strong = composite("#39414d", shade(dark, 600, "#2a4236"), 0.18);
    return {
      ...accent,
      "color.surface": withAlpha(strong, 0.86),
      "color.surface.strong": strong,
      "color.text.primary": "#f5f7f6",
      "color.text.muted": "#9aa5a0",
      "color.canvas.background": canvas,
      "color.panel.background": panel,
      "color.window.titlebar.background": panel,
      "color.window.titlebar.foreground": "#f5f7f6",
      "color.on.wallpaper": "#f5f5f7",
      "color.outline": "rgba(255, 255, 255, 0.14)",
      "color.shadow": "rgba(0, 0, 0, 0.7)"
    };
  }

  return {
    ...accent,
    "color.surface": "#ffffffd9",
    "color.surface.strong": "#ffffff",
    "color.text.primary": "#1d1d1f",
    "color.text.muted": "#6e6e73",
    "color.canvas.background": "#c9c9ce",
    "color.panel.background": "#f5f5f7",
    "color.window.titlebar.background": "#f5f5f7",
    "color.window.titlebar.foreground": "#1d1d1f",
    "color.on.wallpaper": "#ffffff",
    "color.outline": "rgba(0, 0, 0, 0.12)",
    "color.shadow": "rgba(0, 0, 0, 0.16)"
  };
}
