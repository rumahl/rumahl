import { useEffect } from "react";
import { hexToHsl, hexToRgb, hslToHex } from "@rumahl/ui/color";
import { composite } from "@rumahl/ui/palette";
import {
  adaptiveShadow,
  decideInk,
  decideMuted,
  ensureReadableTextColor,
  worstContrast
} from "./adaptive-colors";
import { adaptiveSampleInterval, SAMPLE_STATIC_MS } from "./adaptive-sampling";

const SAMPLE_WIDTH = 320;
const CONFIRM_SWITCH_MS = 900;
const GRID_X = [0.06, 0.28, 0.5, 0.72, 0.94] as const;
const GRID_Y = [0.12, 0.37, 0.63, 0.88] as const;

function hex(value: string, fallback: string): string {
  const match = value.trim().match(/^#?([0-9a-f]{6})/i);
  return match ? `#${match[1]}` : fallback;
}

function rgbHex(r: number, g: number, b: number): string {
  const channel = (value: number) => Math.round(Math.min(Math.max(value, 0), 255)).toString(16).padStart(2, "0");
  return `#${channel(r)}${channel(g)}${channel(b)}`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

interface Rgba { r: number; g: number; b: number; a: number }

function parseColor(value: string): Rgba | null {
  const match = /^rgba?\(([^)]+)\)$/i.exec(value.trim());
  if (!match) return null;
  const parts = match[1]!.split(/[,\s/]+/).filter(Boolean);
  if (parts.length < 3) return null;
  const channel = (part: string) => part.endsWith("%") ? Number.parseFloat(part) * 2.55 : Number.parseFloat(part);
  const [r, g, b] = parts.slice(0, 3).map(channel);
  const a = parts[3] === undefined ? 1 : parts[3].endsWith("%") ? Number.parseFloat(parts[3]) / 100 : Number.parseFloat(parts[3]);
  if (![r, g, b, a].every(Number.isFinite)) return null;
  return { r: r!, g: g!, b: b!, a: clamp(a, 0, 1) };
}

function parseFilters(value: string): { blur: number | null; saturate: number | null; brightness: number | null } {
  const blur = /blur\(([\d.]+)px\)/i.exec(value);
  const saturate = /saturate\(([\d.]+)\)/i.exec(value);
  const brightness = /brightness\(([\d.]+)\)/i.exec(value);
  return {
    blur: blur ? Number.parseFloat(blur[1]!) : null,
    saturate: saturate ? Number.parseFloat(saturate[1]!) : null,
    brightness: brightness ? Number.parseFloat(brightness[1]!) : null
  };
}

function adjustSaturation(hex: string, factor: number): string {
  if (factor === 1) return hex;
  const [hue, saturation, lightness] = hexToHsl(hex);
  return hslToHex(hue, Math.min(100, Math.max(0, saturation * factor)), lightness);
}

function adjustBrightness(hex: string, factor: number): string {
  if (factor === 1) return hex;
  const [r, g, b] = hexToRgb(hex);
  return rgbHex(r * factor, g * factor, b * factor);
}

function mediaElement(): HTMLVideoElement | HTMLImageElement | null {
  const video = document.querySelector<HTMLVideoElement>(".wallpaper-video");
  if (video && video.readyState >= 2 && video.videoWidth > 0) return video;
  const image = document.querySelector<HTMLImageElement>(".wallpaper-image");
  if (image && image.complete && image.naturalWidth > 0) return image;
  return null;
}

interface ToneState {
  ink: string;
  muted: string;
  pendingInk: { color: string; since: number } | null;
  pendingMuted: { color: string; since: number } | null;
}

function stateFor(element: HTMLElement): ToneState {
  const existing = states.get(element);
  if (existing) return existing;
  const state: ToneState = {
    ink: element.style.getPropertyValue("--adaptive-ink").trim(),
    muted: element.style.getPropertyValue("--adaptive-muted").trim(),
    pendingInk: null,
    pendingMuted: null
  };
  states.set(element, state);
  return state;
}

const states = new WeakMap<HTMLElement, ToneState>();

function commitTone(current: string, proposed: string, critical: boolean, pending: ToneState["pendingInk"], now: number) {
  if (!current) return { color: proposed, pending: null };
  if (proposed === current) return { color: current, pending: null };
  if (critical) return { color: proposed, pending: null };
  if (pending && pending.color === proposed && now - pending.since >= CONFIRM_SWITCH_MS) return { color: proposed, pending: null };
  return { color: current, pending: pending?.color === proposed ? pending : { color: proposed, since: now } };
}

/**
 * Shared wallpaper sampling pass. One canvas draw and one ImageData read feed
 * every adaptive text region; each region samples a spread of points rather
 * than a single average. Tone changes use hysteresis plus a confirmation
 * window, and only then the CSS transition animates the visible change.
 */
export function useAdaptiveContrast(): void {
  useEffect(() => {
    if (typeof document === "undefined" || typeof window === "undefined" || typeof HTMLCanvasElement === "undefined") return;
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return;
    let width = 0;
    let height = 0;
    let lastSignature: number[] | null = null;
    let smoothedSceneDelta = 0;
    let timer = 0;

    const resize = () => {
      const next = Math.max(1, Math.min(SAMPLE_WIDTH, window.innerWidth));
      const nextHeight = Math.max(1, Math.round(next * (window.innerHeight / Math.max(1, window.innerWidth))));
      if (next === width && nextHeight === height) return;
      width = next;
      height = nextHeight;
      canvas.width = width;
      canvas.height = height;
    };

    const signatureOf = (data: Uint8ClampedArray): number[] => {
      const cells: number[] = [];
      for (let y = 0; y < 6; y += 1) {
        for (let x = 0; x < 8; x += 1) {
          const px = Math.min(width - 1, Math.floor(((x + 0.5) / 8) * width));
          const py = Math.min(height - 1, Math.floor(((y + 0.5) / 6) * height));
          const index = (py * width + px) * 4;
          cells.push((0.2126 * data[index]! + 0.7152 * data[index + 1]! + 0.0722 * data[index + 2]!) / 255);
        }
      }
      return cells;
    };

    const sampleColors = (data: Uint8ClampedArray, left: number, top: number, right: number, bottom: number, viewportWidth: number, viewportHeight: number): string[] => {
      const colors: string[] = [];
      for (const fy of GRID_Y) {
        for (const fx of GRID_X) {
          const px = clamp(Math.floor((left + (right - left) * fx) / viewportWidth * width), 0, width - 1);
          const py = clamp(Math.floor((top + (bottom - top) * fy) / viewportHeight * height), 0, height - 1);
          const index = (py * width + px) * 4;
          colors.push(rgbHex(data[index]!, data[index + 1]!, data[index + 2]!));
        }
      }
      return colors;
    };

    const tick = (): number => {
      if (document.hidden) return SAMPLE_STATIC_MS;
      const source = mediaElement();
      if (!source) return SAMPLE_STATIC_MS;
      resize();
      const sourceWidth = source instanceof HTMLVideoElement ? source.videoWidth : source.naturalWidth;
      const sourceHeight = source instanceof HTMLVideoElement ? source.videoHeight : source.naturalHeight;
      if (!sourceWidth || !sourceHeight) return SAMPLE_STATIC_MS;
      const scale = Math.max(width / sourceWidth, height / sourceHeight);
      const drawWidth = sourceWidth * scale;
      const drawHeight = sourceHeight * scale;
      let frame: ImageData;
      try {
        ctx.clearRect(0, 0, width, height);
        ctx.drawImage(source, (width - drawWidth) / 2, (height - drawHeight) / 2, drawWidth, drawHeight);
        // The single readback for all text regions. GPU→CPU sync stays bounded
        // even when many widgets are visible.
        frame = ctx.getImageData(0, 0, width, height);
      } catch {
        return SAMPLE_STATIC_MS;
      }

      const signature = signatureOf(frame.data);
      const delta = lastSignature
        ? signature.reduce((sum, value, index) => sum + Math.abs(value - lastSignature![index]!), 0) / signature.length
        : 0;
      lastSignature = signature;
      smoothedSceneDelta = smoothedSceneDelta * 0.55 + delta * 0.45;

      const viewportWidth = Math.max(1, window.innerWidth);
      const viewportHeight = Math.max(1, window.innerHeight);
      const rootStyle = getComputedStyle(document.documentElement);
      const glassAccent = hex(rootStyle.getPropertyValue("--rumahl-ui-color-accent"), "#28694c");
      const materialBlur = Number.parseFloat(rootStyle.getPropertyValue("--rumahl-ui-material-blur")) || 0;
      const materialSaturation = Number.parseFloat(rootStyle.getPropertyValue("--rumahl-ui-material-saturation")) || 1;
      const solid = document.querySelector<HTMLElement>(".shell")?.dataset.material === "solid";
      const now = performance.now();

      for (const element of document.querySelectorAll<HTMLElement>("[data-adaptive-text]")) {
        const glass = element.dataset.adaptiveGlass === "true";
        const surface = glass ? element.querySelector<HTMLElement>(".rumahl-glass-surface") : null;
        const surfaceStyle = surface ? getComputedStyle(surface) : null;
        const surfaceColor = surfaceStyle ? parseColor(surfaceStyle.backgroundColor) : null;
        const filters = parseFilters(surfaceStyle?.getPropertyValue("backdrop-filter") || surfaceStyle?.getPropertyValue("-webkit-backdrop-filter") || "");
        const blur = glass ? filters.blur ?? materialBlur : 0;
        const saturation = glass ? filters.saturate ?? materialSaturation : 1;
        const brightness = glass ? filters.brightness ?? 1 : 1;
        const rect = element.getBoundingClientRect();
        if (rect.width < 1 || rect.height < 1 || rect.bottom < 0 || rect.top > viewportHeight || rect.right < 0 || rect.left > viewportWidth) continue;
        // Blur pulls colour from around a glyph, not only the exact text box.
        const margin = glass ? Math.min(64, blur * 1.5) : 0;
        const left = Math.max(0, rect.left - margin);
        const top = Math.max(0, rect.top - margin);
        const right = Math.min(viewportWidth, rect.right + margin);
        const bottom = Math.min(viewportHeight, rect.bottom + margin);
        const raw = sampleColors(frame.data, left, top, right, bottom, viewportWidth, viewportHeight);
        const backgrounds: string[] = [];
        for (const sample of raw) {
          let background = sample;
          if (glass) {
            background = adjustSaturation(background, saturation);
            background = adjustBrightness(background, brightness);
            if (solid && surfaceColor) {
              background = rgbHex(surfaceColor.r, surfaceColor.g, surfaceColor.b);
            } else {
              if (surfaceColor && surfaceColor.a > 0) {
                background = composite(background, rgbHex(surfaceColor.r, surfaceColor.g, surfaceColor.b), surfaceColor.a);
              } else {
                background = composite(background, "#192036", 0.14);
              }
              background = composite(background, glassAccent, 0.22);
            }
          }
          if (!backgrounds.includes(background)) backgrounds.push(background);
        }
        if (backgrounds.length === 0) continue;

        const state = stateFor(element);
        const desired = element.dataset.adaptiveColor?.trim();
        const inkDecision = desired
          ? (() => {
              // A requested brand/text colour keeps its hue, but its lightness
              // is allowed to move immediately with the scene. It is already
              // worst-case checked, so no light/dark hysteresis is needed.
              const adjusted = ensureReadableTextColor(backgrounds, desired);
              return { color: adjusted.color, ratio: adjusted.ratio, currentRatio: worstContrast(state.ink, backgrounds), critical: true };
            })()
          : decideInk(backgrounds, state.ink);
        const mutedDecision = decideMuted(backgrounds, state.muted);
        const ink = commitTone(state.ink, inkDecision.color, inkDecision.critical, state.pendingInk, now);
        const muted = commitTone(state.muted, mutedDecision.color, mutedDecision.critical, state.pendingMuted, now);
        state.pendingInk = ink.pending;
        state.pendingMuted = muted.pending;

        // Keep the shadow stable. Switching between "strong" and "normal"
        // shadows was visually abrupt and could obscure the glyphs.
        const shadow = adaptiveShadow(backgrounds[0]!);
        const key = `${ink.color}|${muted.color}|${shadow}`;
        if (state.ink !== ink.color || state.muted !== muted.color || element.dataset.adaptiveKey !== key) {
          state.ink = ink.color;
          state.muted = muted.color;
          element.dataset.adaptiveKey = key;
          element.style.setProperty("--adaptive-ink", ink.color);
          element.style.setProperty("--adaptive-muted", muted.color);
          element.style.setProperty("--adaptive-shadow", shadow);
        }
        // Widget frames cannot inherit the shell's custom properties. Send on
        // every pass so frames that loaded late still receive the values.
        for (const frameElement of element.querySelectorAll("iframe")) {
          try { frameElement.contentWindow?.postMessage({ type: "rumahl-adaptive-contrast", ink: ink.color, muted: muted.color }, "*"); } catch { /* cross-origin frames may reject */ }
        }
      }

      const videoPlaying = source instanceof HTMLVideoElement && !source.paused && !source.ended && source.readyState >= 2;
      const reducedMotion = typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      const ultraPerformance = document.querySelector<HTMLElement>(".shell")?.dataset.perf === "ultra";
      return adaptiveSampleInterval({ videoPlaying, sceneDelta: smoothedSceneDelta, reducedMotion, ultraPerformance, hidden: false });
    };

    const loop = () => {
      const delay = tick();
      timer = window.setTimeout(loop, delay);
    };
    loop();
    return () => window.clearTimeout(timer);
  }, []);
}
