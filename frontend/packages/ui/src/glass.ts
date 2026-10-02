import type { Tokens } from "./tokens";

/**
 * Shared glass interface. The engine, materials and CSS hooks are a shell-wide
 * capability — every theme (and extension) can use them, not just the default
 * theme. Themes are data-only, so they interact through:
 *
 *  - the CSS contract below (classes/attributes the shell mounts),
 *  - the `material.*`/`shape.*` design tokens,
 *  - the named materials exported here.
 */
export const GLASS_HOST_CLASS = "rumahl-glass-host";
export const GLASS_SURFACE_CLASS = "rumahl-glass-surface";
export const GLASS_WEBGL_CLASS = "rumahl-glass-webgl";
/** Adds `data-glass="canvas|css"` on `.shell`; the engine mounts the chrome. */
export const GLASS_ENGINE_ATTRIBUTE = "data-glass";

export interface GlassMaterial {
  /** Corner radius (px). */
  radius?: number;
  /** Rim (bevel) width in px. */
  bevel?: number;
  /** Refraction offset at the rim (px). */
  refraction?: number;
  /** Chromatic aberration, 0–0.6. */
  chroma?: number;
  /** Backdrop blur (px). */
  blur?: number;
  /** Backdrop saturation multiplier. */
  saturation?: number;
  /** Backdrop brightness multiplier. */
  brightness?: number;
  /** Milky tint layered over the refracted backdrop. */
  tint?: string;
}

export type GlassBackend = "auto" | "svg" | "webgl" | "css";
export type GlassQuality = "auto" | "high" | "balanced" | "low";

/** Named materials any theme or app may reuse. */
export const GLASS_MATERIALS = {
  dock: { radius: 29, bevel: 22, refraction: 56, chroma: 0.32, blur: 1.2, saturation: 1.05, brightness: 1, tint: "rgba(30,37,66,.05)" },
  widget: { radius: 23, bevel: 20, refraction: 40, chroma: 0.28, blur: 1.2, saturation: 1.05, brightness: 1, tint: "rgba(30,37,66,.05)" },
  bar: { radius: 14, bevel: 16, refraction: 14, chroma: 0.08, blur: 2.1, saturation: 1.05, brightness: 1, tint: "rgba(20,28,47,.10)" }
} as const satisfies Record<string, GlassMaterial>;

/** Derives a glass material from a theme's material tokens. */
export function glassMaterialFromTokens(tokens: Tokens, overrides: GlassMaterial = {}): GlassMaterial {
  const number = (value: string | undefined, fallback: number) => {
    const parsed = Number.parseFloat(value ?? "");
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  return {
    blur: number(tokens["material.blur"], 24) / 10,
    saturation: number(tokens["material.saturation"], 1.5),
    radius: number(tokens["shape.radius.dock"], 24),
    ...overrides
  };
}
