import { contrastRatio, hexToHsl, hexToRgb, hslToHex, relativeLuminance } from "@rumahl/ui/color";

/** Softer inks are preferred as long as they remain comfortably readable. */
const MAIN_TARGET = 5;
const MUTED_TARGET = 4.5;
const CRITICAL_TARGET = 3;
const SWITCH_MARGIN = 0.75;

const MAIN_ON_DARK = ["#9fb1b6", "#b9c8cc", "#d3dfe2", "#eef3f4", "#ffffff"];
const MAIN_ON_LIGHT = ["#5f6d73", "#48555b", "#303a40", "#1a2126", "#0b0d12"];
const MUTED_ON_DARK = ["#93a5aa", "#a6b7bb", "#b9c8cc", "#ccd8db", "#dbe6e8", "#eef3f4", "#ffffff"];
const MUTED_ON_LIGHT = ["#6f7d83", "#5a676d", "#455158", "#303a40"];

export interface ToneDecision {
  color: string;
  ratio: number;
  currentRatio: number;
  /** The current colour is no longer acceptable and should be replaced now. */
  critical: boolean;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function luminance(color: string): number {
  return relativeLuminance(hexToRgb(color));
}

function avgLuminance(backgrounds: readonly string[]): number {
  if (backgrounds.length === 0) return 0;
  return backgrounds.reduce((sum, color) => sum + luminance(color), 0) / backgrounds.length;
}

/** Contrast against the least favourable sampled area, not just the average. */
export function worstContrast(color: string, backgrounds: readonly string[]): number {
  let worst = Number.POSITIVE_INFINITY;
  for (const background of backgrounds) worst = Math.min(worst, contrastRatio(color, background).ratio);
  return Number.isFinite(worst) ? worst : 0;
}

function familyChoice(backgrounds: readonly string[], candidates: readonly string[], target: number): { color: string; ratio: number } {
  let best = candidates[0] ?? "#ffffff";
  let bestRatio = 0;
  for (const candidate of candidates) {
    const ratio = worstContrast(candidate, backgrounds);
    if (ratio >= target) return { color: candidate, ratio };
    if (ratio > bestRatio) { best = candidate; bestRatio = ratio; }
  }
  return { color: best, ratio: bestRatio };
}

function bestTone(backgrounds: readonly string[], darkCandidates: readonly string[], lightCandidates: readonly string[], target: number): { color: string; ratio: number } {
  const lightPreferred = avgLuminance(backgrounds) > 0.35;
  const preferred = familyChoice(backgrounds, lightPreferred ? lightCandidates : darkCandidates, target);
  if (preferred.ratio >= target) return preferred;
  const fallback = familyChoice(backgrounds, lightPreferred ? darkCandidates : lightCandidates, target);
  return fallback.ratio > preferred.ratio ? fallback : preferred;
}

/** Chooses the best readable tone, while keeping the current tone when safe. */
export function decideInk(backgrounds: readonly string[], current?: string, target = MAIN_TARGET): ToneDecision {
  const best = bestTone(backgrounds, MAIN_ON_DARK, MAIN_ON_LIGHT, target);
  if (!current) return { ...best, currentRatio: 0, critical: true };
  const currentRatio = worstContrast(current, backgrounds);
  if (currentRatio >= target && !(best.ratio > currentRatio + 1.25)) {
    return { color: current, ratio: currentRatio, currentRatio, critical: false };
  }
  if (currentRatio < CRITICAL_TARGET || best.ratio >= currentRatio + SWITCH_MARGIN) {
    return { ...best, currentRatio, critical: currentRatio < CRITICAL_TARGET };
  }
  return { color: current, ratio: currentRatio, currentRatio, critical: false };
}

export function decideMuted(backgrounds: readonly string[], current?: string, target = MUTED_TARGET): ToneDecision {
  const best = bestTone(backgrounds, MUTED_ON_DARK, MUTED_ON_LIGHT, target);
  if (!current) return { ...best, currentRatio: 0, critical: true };
  const currentRatio = worstContrast(current, backgrounds);
  if (currentRatio >= target && !(best.ratio > currentRatio + 1.25)) {
    return { color: current, ratio: currentRatio, currentRatio, critical: false };
  }
  if (currentRatio < CRITICAL_TARGET || best.ratio >= currentRatio + SWITCH_MARGIN) {
    return { ...best, currentRatio, critical: currentRatio < CRITICAL_TARGET };
  }
  return { color: current, ratio: currentRatio, currentRatio, critical: false };
}

/**
 * Extracts the opaque colour stops from a CSS background value. Wallpaper
 * gradients are rendered as CSS, not as an <img>, so the adaptive engine needs
 * their stops to reason about label contrast without a canvas readback.
 */
export function colorsInCssBackground(value: string): string[] {
  const colors: string[] = [];
  const matches = value.match(/#[0-9a-f]{3,8}\b|rgba?\([^)]*\)/gi) ?? [];
  for (const match of matches) {
    const color = match.trim();
    if (/^#(?:[0-9a-f]{2}){3}00$/i.test(color)) continue;
    const rgba = /^rgba\(([^)]*)\)$/i.exec(color);
    if (rgba) {
      const parts = rgba[1]!.split(/[,\s/]+/).filter(Boolean);
      if (parts.length >= 4 && Number(parts[3]) === 0) continue;
    }
    if (!colors.some((existing) => existing.toLowerCase() === color.toLowerCase())) colors.push(color);
  }
  return colors;
}

function parseCssColor(value: string): [number, number, number] | null {
  const text = value.trim();
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(text);
  if (hex) {
    const raw = hex[1]!;
    const expanded = raw.length === 3 ? raw.split("").map((part) => part + part).join("") : raw.slice(0, 6);
    return [
      Number.parseInt(expanded.slice(0, 2), 16),
      Number.parseInt(expanded.slice(2, 4), 16),
      Number.parseInt(expanded.slice(4, 6), 16)
    ];
  }
  const rgb = /^rgba?\(([^)]+)\)$/i.exec(text);
  if (!rgb) return null;
  const parts = rgb[1]!.split(/[,\s/]+/).filter(Boolean).map(Number);
  if (parts.length < 3 || !parts.slice(0, 3).every(Number.isFinite)) return null;
  return [parts[0]!, parts[1]!, parts[2]!];
}

/**
 * The representative colour of a CSS wallpaper layer. Unlike a photo, a
 * gradient cannot be sampled per element; its average gives labels a stable
 * local-enough tone while the halo covers mixed stops.
 */
export function averageColor(backgrounds: readonly string[]): string {
  let count = 0;
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const background of backgrounds) {
    const rgb = parseCssColor(background);
    if (!rgb) continue;
    red += rgb[0];
    green += rgb[1];
    blue += rgb[2];
    count += 1;
  }
  if (!count) return "#808080";
  const channel = (value: number) => Math.round(value / count).toString(16).padStart(2, "0");
  return `#${channel(red)}${channel(green)}${channel(blue)}`;
}

/** Picks a readable ink for a single effective backdrop. */
export function adaptiveInk(background: string): string {
  return decideInk([background]).color;
}

export function adaptiveMuted(background: string): string {
  return decideMuted([background]).color;
}

/**
 * Future-proofing for coloured text: keep the requested hue/saturation and
 * only move lightness until the colour clears the target against every sample.
 */
export function ensureReadableTextColor(backgrounds: readonly string[], desired: string, target = MUTED_TARGET): { color: string; ratio: number } {
  const current = worstContrast(desired, backgrounds);
  if (current >= target) return { color: desired, ratio: current };
  const [hue, saturation] = hexToHsl(desired);
  const direction = avgLuminance(backgrounds) < 0.42 ? 1 : -1;
  let best = { color: desired, ratio: current };
  for (const sign of [direction, -direction]) {
    for (let step = 1; step <= 100; step += 1) {
      const lightness = clamp(hexToHsl(desired)[2] + sign * step, 0, 100);
      const color = hslToHex(hue, saturation, lightness);
      const ratio = worstContrast(color, backgrounds);
      if (ratio > best.ratio) best = { color, ratio };
      if (ratio >= target) return { color, ratio };
    }
  }
  return best;
}

/**
 * One stable, subtle halo whose polarity follows the ink, not the wallpaper.
 * When the sampled scene cannot give the ink a safe worst-case contrast, the
 * halo becomes stronger so labels stay legible on patterned backdrops.
 */
export function adaptiveShadow(ink: string, worst = 21): string {
  // The softer light inks on dark glass sit around 0.39 luminance; the old
  // 0.45 cutoff gave them a light halo, which is exactly backwards.
  const lightInk = luminance(ink) > 0.35;
  if (worst >= CRITICAL_TARGET) {
    return lightInk
      ? "0 1px 1px rgba(0, 0, 0, 0.58)"
      : "0 1px 1px rgba(255, 255, 255, 0.7)";
  }
  return lightInk
    ? "0 1px 2px rgba(0, 0, 0, 0.9), 0 0 6px rgba(0, 0, 0, 0.55)"
    : "0 1px 2px rgba(255, 255, 255, 0.95), 0 0 6px rgba(255, 255, 255, 0.65)";
}

