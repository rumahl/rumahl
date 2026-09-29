import { useCallback } from "react";
import { photoTone } from "../wallpapers";
import { withAlpha } from "@rumahl/ui/color";
import { paletteTokens, surfaceContrast } from "@rumahl/ui/palette";
import type { TokenId, Tokens } from "@rumahl/ui/tokens";
import { emptyAppearance, useWorkspace, type AppearanceMode, type AppearanceSettings } from "./Workspace";

export type AppearanceTuning = AppearanceSettings;
export { parseAppearance } from "./Workspace";

/**
 * Appearance customization, stored in the device-scoped workspace so it is
 * resolved server-side and rendered on the first paint (no post-mount flash).
 */
export function useThemeTuning() {
  const workspace = useWorkspace();
  const tuning = workspace.effective.appearance;
  const persist = useCallback((next: AppearanceSettings) => {
    void workspace.save("device", { ...workspace.effective, appearance: next });
  }, [workspace]);
  const setSeed = useCallback((seed: string) => persist({ ...tuning, seed, autoColor: false }), [tuning, persist]);
  const setMode = useCallback((mode: AppearanceMode) => persist({ ...tuning, mode }), [tuning, persist]);
  const setToken = useCallback((token: TokenId, value: string) => persist({ ...tuning, tokens: { ...tuning.tokens, [token]: value } }), [tuning, persist]);
  const setTransparency = useCallback((enabled: boolean) => {
    const tokens = { ...tuning.tokens };
    for (const key of ["material.opacity", "material.blur", "material.saturation", "material.morphism"]) delete tokens[key];
    if (!enabled) Object.assign(tokens, { "material.opacity": "1", "material.blur": "0px", "material.saturation": "1", "material.morphism": "0" });
    persist({ ...tuning, tokens });
  }, [tuning, persist]);
  const setAutoColor = useCallback((autoColor: boolean) => persist({ ...tuning, autoColor }), [tuning, persist]);
  const setWallpaperTint = useCallback((wallpaperTint: boolean) => persist({ ...tuning, wallpaperTint }), [tuning, persist]);
  const setWallpaperMotion = useCallback((wallpaperMotion: boolean) => persist({ ...tuning, wallpaperMotion }), [tuning, persist]);
  const setGlassEngine = useCallback((glassEngine: "css" | "canvas") => persist({ ...tuning, glassEngine }), [tuning, persist]);
  const setGlassEnabled = useCallback((glassEnabled: boolean) => persist({ ...tuning, glassEnabled }), [tuning, persist]);
  const setGlassBackend = useCallback((glassBackend: "auto" | "svg" | "webgl" | "css") => persist({ ...tuning, glassBackend }), [tuning, persist]);
  const setGlassQuality = useCallback((glassQuality: "auto" | "high" | "balanced" | "low") => persist({ ...tuning, glassQuality }), [tuning, persist]);
  const setMaterialPreset = useCallback((preset: "clear" | "soft" | "solid") => {
    const values = { clear: ["0.72", "12px", "1.3", "1"], soft: ["0.9", "16px", "1.1", "0.65"], solid: ["1", "0px", "1", "0"] }[preset];
    persist({ ...tuning, tokens: { ...tuning.tokens, "material.opacity": values[0]!, "material.blur": values[1]!, "material.saturation": values[2]!, "material.morphism": values[3]! } });
  }, [tuning, persist]);
  const reset = useCallback(() => persist(emptyAppearance), [persist]);
  return { tuning, setSeed, setMode, setToken, setTransparency, setAutoColor, setWallpaperTint, setWallpaperMotion, setGlassEngine, setGlassEnabled, setGlassBackend, setGlassQuality, setMaterialPreset, reset };
}

/**
 * Overlays the colour engine and explicit overrides on a theme's tokens, then
 * re-derives readable text from the effective surface. Because the effective
 * surface blends towards the canvas as opacity drops, text contrast adapts
 * automatically when the transparency changes.
 */
export function applyTuning(tokens: Tokens, tuning: AppearanceSettings): Tokens {
  const wallpaper = tuning.tokens["texture.wallpaper"] ?? tokens["texture.wallpaper"];
  const seed = tuning.autoColor ? wallpaperSeed(wallpaper, tokens["color.canvas.background"]) : tuning.seed;
  const customized = seed !== null || tuning.mode !== null;
  const base = customized
    ? { ...tokens, ...paletteTokens(seed ?? tokens["color.accent"], { mode: tuning.mode ?? "light" }) }
    : tokens;
  const merged = { ...base, ...(tuning.tokens as Partial<Tokens>) };
  // The palette already provides readable text. Only a directly chosen surface
  // colour needs an on-the-fly contrast fix-up.
  const explicitSurface = tuning.tokens["color.surface.strong"] !== undefined;
  const contrast = explicitSurface
    ? surfaceContrast(merged["color.surface.strong"], merged["color.canvas.background"], Number.parseFloat(merged["material.opacity"]) || 0.7)
    : null;
  // Wallpaper tinting: surfaces pick up the desktop picture's dominant colour.
  // Independently switchable (default on).
  const tint = tuning.wallpaperTint === false ? "#00000000" : withAlpha(wallpaperSeed(wallpaper, merged["color.canvas.background"]), 0.1);
  return {
    ...merged,
    "color.wallpaper.tint": tint,
    "color.text.primary": tuning.tokens["color.text.primary"] ?? contrast?.textPrimary ?? merged["color.text.primary"],
    "color.text.muted": tuning.tokens["color.text.muted"] ?? contrast?.textMuted ?? merged["color.text.muted"]
  };
}

/** Bounded, deterministic palette extraction also works during SSR. */
export function wallpaperSeed(wallpaper: string, canvas: string): string {
  // Bundled photographs expose a precomputed dominant tone.
  const tone = photoTone(wallpaper);
  if (tone) return tone;
  const colors = wallpaper.match(/#[0-9a-fA-F]{6}\b/g);
  if (!colors?.length) return canvas;
  return "#" + [1, 3, 5].map(offset => Math.round(colors.reduce((sum, color) => sum + Number.parseInt(color.slice(offset, offset + 2), 16), 0) / colors.length).toString(16).padStart(2, "0")).join("");
}
