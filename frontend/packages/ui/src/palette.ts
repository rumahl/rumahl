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
 * Projects the rumahl colour engine onto the theme colour roles. Use this to
 * re-tint the whole OS from a single seed colour, optionally in dark mode.
 * Dark mode is derived entirely from the palette (never pure black), covers
 * every surface role, and derives text contrast from the effective surface.
 */
export function paletteTokens(seed: string, options: PaletteOptions = {}): Partial<Tokens> {
  const colors = themeColorValues(seed, options);
  const palette = generateColorShades(seed, options);
  const dark = options.mode === "dark";
  const canvas = dark ? (findShade(palette.primary.dark, 950)?.hex ?? "#08120f") : "#e8ece7";
  const contrast = surfaceContrast(colors.surfaceStrong, canvas, 0.84);
  const base: Partial<Tokens> = {
    "color.accent": colors.accent,
    "color.accent.strong": colors.accentStrong,
    "color.surface": colors.surface,
    "color.surface.strong": colors.surfaceStrong,
    "color.on.accent": colors.onAccent,
    "color.text.primary": contrast.textPrimary,
    "color.text.muted": contrast.textMuted
  };
  if (dark) {
    return {
      ...base,
      "color.canvas.background": canvas,
      "color.panel.background": findShade(palette.primary.dark, 900)?.hex ?? "#0c1a15",
      "color.window.titlebar.background": findShade(palette.primary.dark, 800)?.hex ?? "#122a22",
      "color.window.titlebar.foreground": contrast.textPrimary,
      "color.on.wallpaper": findShade(palette.primary.light, 170)?.hex ?? contrast.textPrimary,
      "color.outline": withAlpha("#ffffff", 0.18),
      "color.shadow": "rgba(0, 0, 0, 0.5)"
    };
  }
  return {
    ...base,
    "color.canvas.background": canvas,
    "color.panel.background": "#f8faf7",
    "color.window.titlebar.background": "#f2f5f1",
    "color.window.titlebar.foreground": contrast.textPrimary
  };
}
