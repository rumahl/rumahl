import type { AppearanceSettings } from "./Workspace";

export type AppearanceResetGroup = "typography" | "surface" | "glass" | "performance";
const fields = {
  typography: ["adaptiveType"],
  surface: ["autoColor", "wallpaperTint", "wallpaperMotion"],
  glass: ["glassEnabled", "glassReduced", "glassColor", "glassTint", "glassBackend", "glassQuality", "glassEngine"],
  performance: ["performanceMode", "animations"]
} as const;

/** Remove only this group's overrides; defaults come from the active theme. */
export function resetAppearanceGroup(tuning: AppearanceSettings, group: AppearanceResetGroup): AppearanceSettings {
  const next = { ...tuning, tokens: { ...tuning.tokens } };
  for (const field of fields[group]) delete next[field];
  const tokens = group === "typography" ? ["typography.scale"]
    : group === "surface" ? ["material.opacity", "material.blur", "material.saturation", "material.morphism"] : [];
  for (const token of tokens) delete next.tokens[token];
  return next;
}

export function resetThemeParameters(tuning: AppearanceSettings, parameters: readonly { token: string; kind: string; seed?: boolean | undefined }[]): AppearanceSettings {
  const tokens = { ...tuning.tokens };
  for (const parameter of parameters) delete tokens[parameter.token];
  return { ...tuning, tokens, ...(parameters.some(parameter => parameter.kind === "color" && parameter.seed) ? { seed: null } : {}) };
}

export function resetColorOverrides(tuning: AppearanceSettings, tokensToReset: readonly string[]): AppearanceSettings {
  const tokens = { ...tuning.tokens };
  for (const token of tokensToReset) delete tokens[token];
  return { ...tuning, mode: null, tokens };
}
