/**
 * rumahl colour system — the native colour calculation engine.
 *
 * It always derives two tables from a single seed colour:
 *  - the **primary** table (light and dark scales),
 *  - the **secondary** palette (a desaturated companion derived from primary).
 *
 * The engine is deliberately open: every threshold, scale and curve below can be
 * tuned, and callers may pass `vibrancy`/`hueShift`. It is pure and free of any
 * framework dependency so themes, the shell and app SDKs can all share it.
 */

export interface ColorShade {
  shade: number;
  hex: string;
  hue: number;
  saturation: number;
  lightness: number;
}

export interface ContrastResult {
  ratio: number;
  aa: boolean;
  aaa: boolean;
  aaLarge: boolean;
  aaaLarge: boolean;
}

export interface ColorTable {
  light: ColorShade[];
  dark: ColorShade[];
}

export interface ColorPalette {
  primary: ColorTable;
  secondary: ColorTable;
}

export interface PaletteOptions {
  /** Saturation multiplier control (0-85, default 50). */
  vibrancy?: number;
  /** Hue rotation in degrees (default 0). */
  hueShift?: number;
  /** Selects the light table or the dark table. */
  mode?: "light" | "dark";
}

/** Seed colours for the five rumahl semantic roles. */
export const RUMAHL_COLORS = {
  base: "#28694c",
  info: "#0078ff",
  success: "#28a745",
  warning: "#ffc107",
  error: "#dc3545"
} as const;

/** rumahl shade scales. */
export const LIGHT_SHADES = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200];
export const DARK_SHADES = [
  0, 50, 100, 150, 200, 250, 300, 350, 400, 450, 500, 550, 600, 650, 700, 750, 800, 850, 900, 950, 1000
];

/** WCAG contrast thresholds. */
export const WCAG = { AA: 4.5, AAA: 7, AA_LARGE: 3, AAA_LARGE: 4.5 } as const;

export function hexToRgb(hex: string): [number, number, number] {
  const value = hex.replace(/^#/, "");
  return [
    Number.parseInt(value.substring(0, 2), 16),
    Number.parseInt(value.substring(2, 4), 16),
    Number.parseInt(value.substring(4, 6), 16)
  ];
}

export function relativeLuminance(rgb: [number, number, number]): number {
  const linear = (component: number) => {
    const value = component / 255;
    return value <= 0.03928 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2]);
}

export function contrastRatio(color1: string, color2: string): ContrastResult {
  const luminance1 = relativeLuminance(hexToRgb(color1));
  const luminance2 = relativeLuminance(hexToRgb(color2));
  const lighter = Math.max(luminance1, luminance2);
  const darker = Math.min(luminance1, luminance2);
  const ratio = (lighter + 0.05) / (darker + 0.05);
  return {
    ratio,
    aa: ratio >= WCAG.AA,
    aaa: ratio >= WCAG.AAA,
    aaLarge: ratio >= WCAG.AA_LARGE,
    aaaLarge: ratio >= WCAG.AAA_LARGE
  };
}

/** Picks the first list colour that reaches AA contrast on `background`, else the best. */
export function bestContrast(candidates: readonly string[], background: string): string {
  let best = candidates[0] ?? "#ffffff";
  let bestRatio = 0;
  for (const candidate of candidates) {
    const { ratio } = contrastRatio(candidate, background);
    if (ratio >= WCAG.AA) return candidate;
    if (ratio > bestRatio) { best = candidate; bestRatio = ratio; }
  }
  return best;
}

export function hexToHsl(hex: string): [number, number, number] {
  const value = hex.replace(/^#/, "");
  const r = Number.parseInt(value.substring(0, 2), 16) / 255;
  const g = Number.parseInt(value.substring(2, 4), 16) / 255;
  const b = Number.parseInt(value.substring(4, 6), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const lightness = (max + min) / 2;
  let hue = 0;
  let saturation = 0;
  if (max !== min) {
    saturation = lightness > 0.5 ? (max - min) / (2 - max - min) : (max - min) / (max + min);
    if (max === r) hue = (g - b) / (max - min) + (g < b ? 6 : 0);
    else if (max === g) hue = (b - r) / (max - min) + 2;
    else hue = (r - g) / (max - min) + 4;
    hue /= 6;
  }
  return [Math.round(hue * 360), Math.round(saturation * 100), Math.round(lightness * 100)];
}

export function hslToHex(hue: number, saturation: number, lightness: number): string {
  const h = hue / 360;
  const s = saturation / 100;
  const l = lightness / 100;
  let r: number;
  let g: number;
  let b: number;
  if (s === 0) {
    r = g = b = l;
  } else {
    const hue2rgb = (p: number, q: number, t: number) => {
      let value = t;
      if (value < 0) value += 1;
      if (value > 1) value -= 1;
      if (value < 1 / 6) return p + (q - p) * 6 * value;
      if (value < 1 / 2) return q;
      if (value < 2 / 3) return p + (q - p) * (2 / 3 - value) * 6;
      return p;
    };
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    r = hue2rgb(p, q, h + 1 / 3);
    g = hue2rgb(p, q, h);
    b = hue2rgb(p, q, h - 1 / 3);
  }
  const toHex = (x: number) => {
    const hex = Math.round(x * 255).toString(16);
    return hex.length === 1 ? `0${hex}` : hex;
  };
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`.toUpperCase();
}

export function findShade(shades: readonly ColorShade[], shade: number): ColorShade | undefined {
  return shades.find((entry) => entry.shade === shade);
}

/** Appends an alpha channel to a `#rrggbb` colour. */
export function withAlpha(hex: string, alpha: number): string {
  const rounded = Math.round(Math.min(Math.max(alpha, 0), 1) * 255).toString(16).padStart(2, "0");
  const base = /^#([0-9a-fA-F]{6})/.exec(hex)?.[1]?.toLowerCase() ?? "000000";
  return `#${base}${rounded}`;
}

/**
 * Generates the rumahl primary table and secondary palette from a seed colour.
 * Secondary shades are derived from primary light 110 so the two tables always
 * share the same hue family.
 */
export function generateColorShades(seed: string, options: PaletteOptions = {}): ColorPalette {
  const [baseHue, baseSat] = hexToHsl(seed);
  const adjustedHue = (baseHue + (options.hueShift ?? 0) + 360) % 360;
  const cappedVibrancy = Math.min(options.vibrancy ?? 50, 85);
  const saturationMultiplier = cappedVibrancy / 50;

  const primaryLightShades = LIGHT_SHADES.map((shade) => {
    let targetLightness: number;
    let adjustedSaturation: number;
    if (shade <= 50) {
      targetLightness = 20 + (shade / 50) * 30;
      adjustedSaturation = baseSat * saturationMultiplier * 0.95;
    } else if (shade <= 100) {
      targetLightness = 50 + ((shade - 50) / 50) * 20;
      adjustedSaturation = baseSat * saturationMultiplier * 0.85;
    } else if (shade <= 150) {
      targetLightness = 70 + ((shade - 100) / 50) * 20;
      adjustedSaturation = baseSat * saturationMultiplier * 0.7;
    } else {
      targetLightness = 90 + ((shade - 150) / 50) * 9;
      adjustedSaturation = baseSat * saturationMultiplier * 0.4;
    }
    adjustedSaturation = Math.min(adjustedSaturation, 100);
    targetLightness = Math.min(targetLightness, 99);
    return {
      shade,
      hex: hslToHex(adjustedHue, adjustedSaturation, targetLightness),
      hue: adjustedHue,
      saturation: Math.round(adjustedSaturation),
      lightness: Math.round(targetLightness)
    };
  });

  const primaryDarkShades = DARK_SHADES.map((shade, index) => {
    const normalizedShade = Math.floor((index / 20) * 200);
    let targetLightness: number;
    let adjustedSaturation: number;
    if (normalizedShade <= 50) {
      targetLightness = 35 - (normalizedShade / 50) * 10;
      adjustedSaturation = baseSat * saturationMultiplier * 0.9;
    } else if (normalizedShade <= 100) {
      targetLightness = 25 - ((normalizedShade - 50) / 50) * 10;
      adjustedSaturation = baseSat * saturationMultiplier * 0.85;
    } else if (normalizedShade <= 150) {
      targetLightness = 15 - ((normalizedShade - 100) / 50) * 10;
      adjustedSaturation = baseSat * saturationMultiplier * 0.7;
    } else {
      targetLightness = 5 - ((normalizedShade - 150) / 50) * 4;
      adjustedSaturation = baseSat * saturationMultiplier * 0.5;
    }
    targetLightness = Math.max(targetLightness, 1);
    return {
      shade,
      hex: hslToHex(adjustedHue, adjustedSaturation, targetLightness),
      hue: adjustedHue,
      saturation: Math.round(adjustedSaturation),
      lightness: Math.round(targetLightness)
    };
  });

  const secondaryBase = findShade(primaryLightShades, 110)?.hex ?? seed;
  const [secondaryHue, secondarySat] = hexToHsl(secondaryBase);

  const secondaryLightShades = LIGHT_SHADES.map((shade) => {
    let targetLightness: number;
    let adjustedSaturation: number;
    if (shade <= 50) {
      targetLightness = 18 + (shade / 50) * 30;
      adjustedSaturation = secondarySat * 0.8;
    } else if (shade <= 100) {
      targetLightness = 48 + ((shade - 50) / 50) * 20;
      adjustedSaturation = secondarySat * 0.7;
    } else if (shade <= 150) {
      targetLightness = 68 + ((shade - 100) / 50) * 20;
      adjustedSaturation = secondarySat * 0.6;
    } else {
      targetLightness = 88 + ((shade - 150) / 50) * 11;
      adjustedSaturation = secondarySat * 0.3;
    }
    targetLightness = Math.min(targetLightness, 99);
    return {
      shade,
      hex: hslToHex(secondaryHue, adjustedSaturation, targetLightness),
      hue: secondaryHue,
      saturation: Math.round(adjustedSaturation),
      lightness: Math.round(targetLightness)
    };
  });

  const secondaryDarkShades = DARK_SHADES.map((shade, index) => {
    const normalizedShade = Math.floor((index / 20) * 200);
    let targetLightness: number;
    let adjustedSaturation: number;
    if (normalizedShade <= 50) {
      targetLightness = 30 - (normalizedShade / 50) * 8;
      adjustedSaturation = secondarySat * 0.75;
    } else if (normalizedShade <= 100) {
      targetLightness = 22 - ((normalizedShade - 50) / 50) * 8;
      adjustedSaturation = secondarySat * 0.7;
    } else if (normalizedShade <= 150) {
      targetLightness = 14 - ((normalizedShade - 100) / 50) * 8;
      adjustedSaturation = secondarySat * 0.6;
    } else {
      targetLightness = 6 - ((normalizedShade - 150) / 50) * 4;
      adjustedSaturation = secondarySat * 0.4;
    }
    targetLightness = Math.max(targetLightness, 1);
    return {
      shade,
      hex: hslToHex(secondaryHue, adjustedSaturation, targetLightness),
      hue: secondaryHue,
      saturation: Math.round(adjustedSaturation),
      lightness: Math.round(targetLightness)
    };
  });

  return {
    primary: { light: primaryLightShades, dark: primaryDarkShades },
    secondary: { light: secondaryLightShades, dark: secondaryDarkShades }
  };
}

export interface ThemeColorValues {
  accent: string;
  accentStrong: string;
  surface: string;
  surfaceStrong: string;
  onAccent: string;
}

/**
 * Projects the generated palette onto the colour roles the shell consumes, so
 * the official theme's colours are always derived from its seed colour.
 */
export function themeColorValues(seed: string, options: PaletteOptions = {}): ThemeColorValues {
  const palette = generateColorShades(seed, options);
  const onAccent = bestContrast(["#ffffff", "#101418"], seed);
  if (options.mode === "dark") {
    // A soft, palette-derived dark (not pure black).
    const surfaceStrong = findShade(palette.primary.dark, 850)?.hex ?? "#0e1a16";
    return {
      accent: seed,
      accentStrong: findShade(palette.primary.light, 60)?.hex ?? seed,
      surfaceStrong,
      surface: withAlpha(surfaceStrong, 0.84),
      onAccent
    };
  }
  const surfaceStrong = findShade(palette.primary.light, 200)?.hex ?? seed;
  return {
    accent: seed,
    accentStrong: findShade(palette.primary.light, 60)?.hex ?? seed,
    surfaceStrong,
    surface: withAlpha(surfaceStrong, 0.84),
    onAccent
  };
}
