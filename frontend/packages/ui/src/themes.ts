import { paletteTokens } from "./palette";
import { defaultTokens, FONT_STACKS, mergeTokens, tokensForBase, type TokenId, type Tokens } from "./tokens";
import { defaultVariants, resolveVariants, type VariantSelection } from "./variants";
import { sanitizeSlots, type SlotName, type SlotTemplate } from "./slots";


/**
 * A tunable theme parameter. Themes declare their own parameters; the shell
 * renders a control per `kind` and writes the resulting token override into the
 * user's appearance tuning. `labelKey`/`option.labelKey` are resolved by i18n.
 */
export type ThemeLabel = string | { en?: string; de?: string };

interface ParameterBase {
  id: string;
  /** Any token id; theme packages may define their own. */
  token: string;
  /** Built-in i18n key. */
  labelKey?: string;
  /** Literal label, optionally per locale, shipped by a theme package. */
  label?: ThemeLabel;
}

export interface RangeParameter extends ParameterBase {
  kind: "range";
  min: number;
  max: number;
  step: number;
  /** Suffix for package-defined ranges whose format lives in JSON. */
  unit?: string | undefined;
  /** Fixed decimal places for package-defined ranges (e.g. 2 for 0.72). */
  decimals?: number | undefined;
  format: (value: number) => string;
  parse: (value: string) => number;
}

export interface ColorParameter extends ParameterBase {
  kind: "color";
  /** When true the control seeds the colour engine and recomputes derived roles. */
  seed?: boolean | undefined;
  /** Directly selectable colours. `id` maps to a CSS swatch class. */
  presets?:
    | readonly { id: string; value: string; labelKey?: string; label?: ThemeLabel }[]
    | undefined;
}

export type AppearanceMode = "light" | "dark";

export interface ChoiceParameter extends ParameterBase {
  kind: "choice";
  options: readonly { value: string; labelKey?: string; label?: ThemeLabel }[];
}

export type ThemeParameter = RangeParameter | ColorParameter | ChoiceParameter;

/**
 * A resolved theme is plain data: token values plus the variant selection and
 * the theme's own customization parameters.
 */
/**
 * Base look a theme builds on.
 *
 * - `iris` is the native rumahl OS base: native chrome, glass and the shared
 *   component language.
 * - `ground` is the open foundation: square, flat, shadow-less primitives for
 *   a theme that wants to build its own presentation without fighting Iris.
 *   It is a base, not a theme.
 *
 * Legacy manifests may still say `rumahl` (Iris) or `clean` (Ground); they are
 * normalized at registration time.
 */
export type ThemeBase = "iris" | "ground";

/** Normalize a persisted/legacy theme base onto the current names. */
export function normalizeThemeBase(value: unknown): ThemeBase | undefined {
  if (value === "ground" || value === "clean") return "ground";
  if (value === "iris" || value === "rumahl") return "iris";
  return undefined;
}

export interface Theme {
  id: string;
  name: string;
  tokens: Tokens;
  variants: VariantSelection;
  parameters: readonly ThemeParameter[];
  /** Appearance modes the theme supports (a theme may be light-only). */
  modes: readonly AppearanceMode[];
  /** Optional per-region markup the theme provides as safe templates. */
  slots?: Partial<Record<SlotName, SlotTemplate>> | undefined;
  /** Optional theme stylesheet (applied CSP-safely, first-party themes). */
  css?: string | undefined;
  /** Optional built-in preset families. These are curated by the theme, not a
   * generic shell capability; themes that omit them get only their parameters. */
  presets?: { look?: boolean; material?: boolean } | undefined;
  /** Fixed opaque material, even when device settings request transparency. */
  material?: "solid" | undefined;
  /** Base look (defaults to `iris`). */
  base?: ThemeBase | undefined;
  /** Fixed art direction: device colour/material tuning is ignored. */
  locked?: boolean | undefined;
  /** Declared capabilities for code themes (`network`, `storage`, …). */
  capabilities?: readonly string[] | undefined;
  /** Custom code (module source) the shell applies for a trusted theme. */
  code?: string | undefined;
  /** Optional real preview image (data URI or bundled asset). */
  preview?: string | undefined;
  /** Where the theme came from; installed packages set this on registration. */
  origin?: "builtin" | "store" | "manual" | undefined;
}

function px(
  token: TokenId,
  id: string,
  labelKey: string,
  fallback: number,
  max: number,
): RangeParameter {
  return {
    kind: "range",
    id,
    token,
    labelKey,
    min: 0,
    max,
    step: 1,
    format: (value) => `${Math.round(value)}px`,
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : fallback;
    },
  };
}

const WALLPAPERS: readonly { value: string; labelKey: string }[] = [
  { value: "default", labelKey: "theme.wallpaper.default" },
  { value: "photo:rodenn", labelKey: "theme.wallpaper.photoRoden" },
  { value: "photo:towner", labelKey: "theme.wallpaper.photoTowner" },
  { value: "photo:jonny", labelKey: "theme.wallpaper.photoJonny" },
  { value: "none", labelKey: "theme.wallpaper.none" },
  {
    value: "linear-gradient(160deg, #0f2027, #203a43, #2c5364)",
    labelKey: "theme.wallpaper.deep",
  },
  {
    value: "linear-gradient(160deg, #1a2a6c, #b21f1f, #fdbb2d)",
    labelKey: "theme.wallpaper.sunset",
  },
  {
    value: "linear-gradient(160deg, #134e5e, #71b280)",
    labelKey: "theme.wallpaper.forest",
  },
  {
    value: "linear-gradient(160deg, #42275a, #734b6d)",
    labelKey: "theme.wallpaper.plum",
  },
];

/** Selectable typography variants (device-resolved system stacks). */
const FONTS: readonly { value: string; labelKey: string }[] = [
  { value: FONT_STACKS.modern, labelKey: "theme.font.modern" },
  { value: FONT_STACKS.system, labelKey: "theme.font.system" },
  { value: FONT_STACKS.neutral, labelKey: "theme.font.neutral" },
  { value: FONT_STACKS.rounded, labelKey: "theme.font.rounded" },
  { value: FONT_STACKS.serif, labelKey: "theme.font.serif" },
  { value: FONT_STACKS.mono, labelKey: "theme.font.mono" },
  { value: FONT_STACKS.manrope, labelKey: "theme.font.manrope" },
  { value: FONT_STACKS.plusJakartaSans, labelKey: "theme.font.plusJakartaSans" },
];

/** Bundled display faces for the launcher greeting (statement typography). */
const DISPLAY_FONTS: readonly { value: string; labelKey: string }[] = [
  { value: FONT_STACKS.spaceGrotesk, labelKey: "theme.font.spaceGrotesk" },
  { value: FONT_STACKS.clashDisplay, labelKey: "theme.font.clashDisplay" },
  { value: FONT_STACKS.bricolageGrotesque, labelKey: "theme.font.bricolageGrotesque" },
  { value: FONT_STACKS.instrumentSerif, labelKey: "theme.font.instrumentSerif" },
  { value: FONT_STACKS.outfit, labelKey: "theme.font.outfit" },
  { value: FONT_STACKS.sora, labelKey: "theme.font.sora" },
  { value: FONT_STACKS.syne, labelKey: "theme.font.syne" },
  { value: FONT_STACKS.unbounded, labelKey: "theme.font.unbounded" },
  { value: FONT_STACKS.fraunces, labelKey: "theme.font.fraunces" },
  { value: FONT_STACKS.playfairDisplay, labelKey: "theme.font.playfairDisplay" },
  { value: FONT_STACKS.dmSerifDisplay, labelKey: "theme.font.dmSerifDisplay" },
  { value: FONT_STACKS.gabarito, labelKey: "theme.font.gabarito" },
];

/** Directly selectable accent colours. */
const COLOR_PRESETS = [
  { id: "red", value: "#e5484d", labelKey: "theme.color.red" },
  { id: "yellow", value: "#eab308", labelKey: "theme.color.yellow" },
  { id: "green", value: "#2f9e6b", labelKey: "theme.color.green" },
  { id: "blue", value: "#3b82f6", labelKey: "theme.color.blue" },
  { id: "purple", value: "#8b5cf6", labelKey: "theme.color.purple" },
] as const;

/** Parameters offered by the official rumahl theme (colour, wallpaper, glass). */
const officialParameters: readonly ThemeParameter[] = [
  {
    kind: "color",
    id: "accent",
    token: "color.accent",
    labelKey: "theme.parameter.accent",
    seed: true,
    presets: COLOR_PRESETS,
  },
  {
    kind: "color",
    id: "panel",
    token: "color.surface.strong",
    labelKey: "theme.parameter.panel",
  },
  {
    kind: "color",
    id: "text",
    token: "color.text.primary",
    labelKey: "theme.parameter.text",
  },
  {
    kind: "choice",
    id: "wallpaper",
    token: "texture.wallpaper",
    labelKey: "theme.parameter.wallpaper",
    options: WALLPAPERS,
  },
  {
    kind: "choice",
    id: "font",
    token: "typography.family",
    labelKey: "theme.parameter.font",
    options: FONTS,
  },
  {
    kind: "choice",
    id: "displayFont",
    token: "typography.display",
    labelKey: "theme.parameter.displayFont",
    options: DISPLAY_FONTS,
  },
  {
    kind: "choice",
    id: "appInputs",
    token: "app.inputs",
    labelKey: "theme.parameter.appInputs",
    options: [
      { value: "glass", labelKey: "theme.parameter.appInputs.glass" },
      { value: "solid", labelKey: "theme.parameter.appInputs.solid" },
    ],
  },
  {
    kind: "range",
    id: "morphism",
    token: "material.morphism",
    labelKey: "theme.parameter.morphism",
    min: 0,
    max: 1,
    step: 0.05,
    format: (value) => value.toFixed(2),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 1;
    },
  },
  {
    kind: "range",
    id: "opacity",
    token: "material.opacity",
    labelKey: "theme.parameter.opacity",
    min: 0.2,
    max: 1,
    step: 0.02,
    format: (value) => value.toFixed(2),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 0.7;
    },
  },
  px("material.blur", "blur", "theme.parameter.blur", 60, 120),
  {
    kind: "range",
    id: "saturation",
    token: "material.saturation",
    labelKey: "theme.parameter.saturation",
    min: 1,
    max: 3,
    step: 0.1,
    format: (value) => value.toFixed(1),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 1.8;
    },
  },
  px("shape.radius.dock", "radius", "theme.parameter.radius", 28, 40),
  px("shape.radius.window", "window", "theme.parameter.window", 23, 32),
  px("shape.radius.icon", "icon", "theme.parameter.icon", 18, 30),
];

/** The rumahl brand default: green accent, glass materials, dock + springboard. */
export const rumahlTheme: Theme = {
  id: "com.rumahl.default",
  name: "Iris",
  base: "iris",
  tokens: { ...defaultTokens, ...paletteTokens(defaultTokens["color.accent"]) },
  variants: defaultVariants,
  parameters: officialParameters,
  presets: { look: true, material: true },
  modes: ["light", "dark"],
  origin: "builtin",
};

/** Ranges coming from a theme package have no functions; provide sane defaults. */
export function normalizeParameter(parameter: ThemeParameter): ThemeParameter {
  if (parameter.kind !== "range") return parameter;
  return {
    ...parameter,
    format: typeof parameter.format === "function"
      ? parameter.format
      : (value: number) =>
          parameter.decimals !== undefined
            ? `${value.toFixed(parameter.decimals)}${parameter.unit ?? ""}`
            : `${Math.round(value)}${parameter.unit ?? ""}`,
    parse: typeof parameter.parse === "function"
      ? parameter.parse
      : (value: string) => {
          const parsed = Number.parseFloat(value);
          return Number.isFinite(parsed) ? parsed : parameter.min;
        },
  };
}

/** The shape of `theme.json` inside a theme package. */
export interface ThemePackageManifest {
  id: string;
  name: string;
  description?: string | undefined;
  version?: string | undefined;
  author?: string | undefined;
  locked?: boolean | undefined;
  modes?: readonly AppearanceMode[] | undefined;
  variants?: Partial<VariantSelection> | undefined;
  tokens?: Partial<Tokens> | undefined;
  parameters?: readonly ThemeParameter[] | undefined;
  preview?: string | undefined;
  slots?: Partial<Record<SlotName, SlotTemplate>> | undefined;
  material?: "solid" | undefined;
  base?: ThemeBase | "rumahl" | "clean" | undefined;
}

/**
 * Turns a theme package (`theme.json` + optional stylesheet and code) into a
 * normal theme. This is the same path an installed ZIP takes.
 */
export function themeFromPackage(manifest: ThemePackageManifest, css?: string, code?: string): Theme {
  const base = normalizeThemeBase(manifest.base) ?? "iris";
  return {
    id: manifest.id,
    name: manifest.name,
    tokens: mergeTokens(tokensForBase(base), manifest.tokens),
    variants: resolveVariants(manifest.variants),
    parameters: (manifest.parameters ?? []).map(normalizeParameter),
    modes: manifest.modes ?? ["light"],
    ...(manifest.locked ? { locked: true } : {}),
    ...(manifest.slots ? { slots: sanitizeSlots(manifest.slots) } : {}),
    ...(manifest.material ? { material: manifest.material } : {}),
    ...(base === "iris" && !manifest.base ? {} : { base }),
    ...(css ? { css } : {}),
    ...(code ? { code } : {}),
    ...(manifest.preview ? { preview: manifest.preview } : {}),
  };
}

export const themes = [rumahlTheme] as const;

export const defaultTheme = rumahlTheme;
export const DEFAULT_THEME_ID = rumahlTheme.id;
