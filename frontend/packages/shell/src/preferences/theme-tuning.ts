import { useCallback } from "react";
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
  const setSeed = useCallback((seed: string) => persist({ ...tuning, seed }), [tuning, persist]);
  const setMode = useCallback((mode: AppearanceMode) => persist({ ...tuning, mode }), [tuning, persist]);
  const setToken = useCallback((token: TokenId, value: string) => persist({ ...tuning, tokens: { ...tuning.tokens, [token]: value } }), [tuning, persist]);
  const reset = useCallback(() => persist(emptyAppearance), [persist]);
  return { tuning, setSeed, setMode, setToken, reset };
}

/**
 * Overlays the colour engine and explicit overrides on a theme's tokens, then
 * re-derives readable text from the effective surface. Because the effective
 * surface blends towards the canvas as opacity drops, text contrast adapts
 * automatically when the transparency changes.
 */
export function applyTuning(tokens: Tokens, tuning: AppearanceSettings): Tokens {
  const customized = tuning.seed !== null || tuning.mode !== null;
  const base = customized
    ? { ...tokens, ...paletteTokens(tuning.seed ?? tokens["color.accent"], { mode: tuning.mode ?? "light" }) }
    : tokens;
  const merged = { ...base, ...(tuning.tokens as Partial<Tokens>) };
  const opacity = Number.parseFloat(merged["material.opacity"]);
  const contrast = surfaceContrast(merged["color.surface.strong"], merged["color.canvas.background"], Number.isFinite(opacity) ? opacity : 0.84);
  return {
    ...merged,
    "color.text.primary": tuning.tokens["color.text.primary"] ?? contrast.textPrimary,
    "color.text.muted": tuning.tokens["color.text.muted"] ?? contrast.textMuted
  };
}
