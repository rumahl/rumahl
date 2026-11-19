import { emptyAppearance, type AppearanceSettings } from "./Workspace";

export interface AppearancePreset {
  id: string;
  labelKey: "theme.look.iris" | "theme.look.lime" | "theme.look.ocean" | "theme.look.rose" | "theme.look.amber";
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
 * One-click appearance looks layered over the Iris theme.
 *
 * Iris is the native default: the theme's own green seed, Inter/Space Grotesk
 * and component language. The other looks change the accent and material only;
 * they are tunings of Iris, not separate themes or identities.
 */
export const appearancePresets: readonly AppearancePreset[] = [
  {
    id: "iris",
    labelKey: "theme.look.iris",
    appearance: { ...emptyAppearance, tokens: {} }
  },
  {
    // The former lime/Manrope/Outfit demo tuning. It remains available as an
    // optional look, but it is not a second theme or default identity.
    id: "lime",
    labelKey: "theme.look.lime",
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

/** The native Iris look: the theme defaults with no appearance override. */
export const irisLook = appearancePresets[0]!.appearance;

const KEYS = [
  "autoColor", "wallpaperTint", "wallpaperMotion", "glassReduced", "animations",
  "performanceMode", "ultraPerformance", "glassEngine", "glassEnabled", "glassBackend",
  "glassQuality", "seed"
] as const satisfies readonly (keyof AppearanceSettings)[];

function tokensKey(tokens: AppearanceSettings["tokens"]): string {
  return JSON.stringify(Object.keys(tokens)
    .filter((key) => key !== "texture.wallpaper" && key !== "iris.controls")
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
  const controls = current.tokens["iris.controls"];
  if (controls !== undefined) tokens["iris.controls"] = controls;
  return {
    ...preset,
    mode: current.mode,
    ...(current.glassBackend !== undefined ? { glassBackend: current.glassBackend } : {}),
    ...(current.glassQuality !== undefined ? { glassQuality: current.glassQuality } : {}),
    ...(current.glassEnabled !== undefined ? { glassEnabled: current.glassEnabled } : {}),
    ...(current.glassReduced !== undefined ? { glassReduced: current.glassReduced } : {}),
    ...(current.mobileNavSvgGlass !== undefined ? { mobileNavSvgGlass: current.mobileNavSvgGlass } : {}),
    ...(current.glassColor !== undefined ? { glassColor: current.glassColor } : {}),
    ...(current.glassTint !== undefined ? { glassTint: current.glassTint } : {}),
    tokens
  };
}

export function isLookActive(current: AppearanceSettings, preset: AppearanceSettings): boolean {
  return KEYS.every((key) => current[key] === preset[key]) && tokensKey(current.tokens) === tokensKey(preset.tokens);
}
