import { emptyAppearance, type AppearanceSettings } from "./Workspace";

export interface AppearancePreset {
  id: string;
  labelKey: "theme.look.aura" | "theme.look.neutral" | "theme.look.ocean" | "theme.look.rose" | "theme.look.amber";
  appearance: AppearanceSettings;
}

const LIME_TOKENS = {
  "app.inputs": "glass",
  "material.blur": "12px",
  "material.morphism": "1",
  "material.opacity": "0.72",
  "material.saturation": "1.3",
  "typography.display": '"Outfit Variable", system-ui, sans-serif',
  "typography.family": '"Manrope Variable", system-ui, sans-serif'
};

/**
 * One-click appearance looks. The first entry is the Aura default shown by the
 * public demo: lime accent, dark scheme and the local canvas material.
 */
export const appearancePresets: readonly AppearancePreset[] = [
  {
    id: "aura",
    labelKey: "theme.look.aura",
    appearance: {
      ...emptyAppearance,
      seed: "#beff2d",
      mode: null,
      glassEngine: "canvas",
      glassBackend: "auto",
      glassQuality: "auto",
      animations: false,
      wallpaperMotion: false,
      tokens: LIME_TOKENS
    }
  },
  {
    id: "neutral",
    labelKey: "theme.look.neutral",
    appearance: { ...emptyAppearance, tokens: {} }
  },
  {
    id: "ocean",
    labelKey: "theme.look.ocean",
    appearance: { ...emptyAppearance, seed: "#0b84ff", mode: null, tokens: {} }
  },
  {
    id: "rose",
    labelKey: "theme.look.rose",
    appearance: { ...emptyAppearance, seed: "#f45b85", mode: null, tokens: {} }
  },
  {
    id: "amber",
    labelKey: "theme.look.amber",
    appearance: { ...emptyAppearance, seed: "#eab308", mode: null, tokens: {} }
  }
];

/** Shared default used by the public demo seed. */
export const defaultLook = appearancePresets[0]!.appearance;

const KEYS = [
  "autoColor", "wallpaperTint", "wallpaperMotion", "glassReduced", "animations",
  "performanceMode", "ultraPerformance", "glassEngine", "glassEnabled", "glassBackend",
  "glassQuality", "seed"
] as const satisfies readonly (keyof AppearanceSettings)[];

function tokensKey(tokens: AppearanceSettings["tokens"]): string {
  return JSON.stringify(Object.keys(tokens)
    .filter((key) => key !== "texture.wallpaper")
    .sort()
    .map((key) => [key, tokens[key]]));
}

/**
 * Presets never change the user's light/dark choice or wallpaper. They only
 * apply the accent and the material tokens of the look.
 */
export function applyLook(current: AppearanceSettings, preset: AppearanceSettings): AppearanceSettings {
  const tokens = { ...preset.tokens };
  const wallpaper = current.tokens["texture.wallpaper"];
  if (wallpaper !== undefined) tokens["texture.wallpaper"] = wallpaper;
  return {
    ...preset,
    mode: current.mode,
    ...(current.glassColor !== undefined ? { glassColor: current.glassColor } : {}),
    ...(current.glassTint !== undefined ? { glassTint: current.glassTint } : {}),
    tokens
  };
}

export function isLookActive(current: AppearanceSettings, preset: AppearanceSettings): boolean {
  return KEYS.every((key) => current[key] === preset[key]) && tokensKey(current.tokens) === tokensKey(preset.tokens);
}
