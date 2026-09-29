import { paletteTokens } from "./palette";
import { themeColorValues } from "./color";
import { defaultTokens, type TokenId, type Tokens } from "./tokens";
import { defaultVariants, type VariantSelection } from "./variants";

const CLASSIC_COLORS = themeColorValues("#3465a4");

/**
 * A tunable theme parameter. Themes declare their own parameters; the shell
 * renders a control per `kind` and writes the resulting token override into the
 * user's appearance tuning. `labelKey`/`option.labelKey` are resolved by i18n.
 */
interface ParameterBase {
  id: string;
  token: TokenId;
  labelKey: string;
}

export interface RangeParameter extends ParameterBase {
  kind: "range";
  min: number;
  max: number;
  step: number;
  format: (value: number) => string;
  parse: (value: string) => number;
}

export interface ColorParameter extends ParameterBase {
  kind: "color";
  /** When true the control seeds the colour engine and recomputes derived roles. */
  seed?: boolean | undefined;
  /** Directly selectable colours. `id` maps to a CSS swatch class. */
  presets?: readonly { id: string; value: string; labelKey: string }[] | undefined;
}

export type AppearanceMode = "light" | "dark";

export interface ChoiceParameter extends ParameterBase {
  kind: "choice";
  options: readonly { value: string; labelKey: string }[];
}

export type ThemeParameter = RangeParameter | ColorParameter | ChoiceParameter;

/**
 * A resolved theme is plain data: token values plus the variant selection and
 * the theme's own customization parameters.
 */
export interface Theme {
  id: string;
  name: string;
  tokens: Tokens;
  variants: VariantSelection;
  parameters: readonly ThemeParameter[];
  /** Appearance modes the theme supports (a theme may be light-only). */
  modes: readonly AppearanceMode[];
}

function px(token: TokenId, id: string, labelKey: string, fallback: number, max: number): RangeParameter {
  return {
    kind: "range", id, token, labelKey, min: 0, max, step: 1,
    format: (value) => `${Math.round(value)}px`,
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : fallback;
    }
  };
}

const WALLPAPERS: readonly { value: string; labelKey: string }[] = [
  { value: "default", labelKey: "theme.wallpaper.default" },
  { value: "photo:rodenn", labelKey: "theme.wallpaper.photoRoden" },
  { value: "photo:towner", labelKey: "theme.wallpaper.photoTowner" },
  { value: "photo:jonny", labelKey: "theme.wallpaper.photoJonny" },
  { value: "none", labelKey: "theme.wallpaper.none" },
  { value: "linear-gradient(160deg, #0f2027, #203a43, #2c5364)", labelKey: "theme.wallpaper.deep" },
  { value: "linear-gradient(160deg, #1a2a6c, #b21f1f, #fdbb2d)", labelKey: "theme.wallpaper.sunset" },
  { value: "linear-gradient(160deg, #134e5e, #71b280)", labelKey: "theme.wallpaper.forest" },
  { value: "linear-gradient(160deg, #42275a, #734b6d)", labelKey: "theme.wallpaper.plum" }
];

/** Directly selectable accent colours. */
const COLOR_PRESETS = [
  { id: "red", value: "#e5484d", labelKey: "theme.color.red" },
  { id: "yellow", value: "#eab308", labelKey: "theme.color.yellow" },
  { id: "green", value: "#2f9e6b", labelKey: "theme.color.green" },
  { id: "blue", value: "#3b82f6", labelKey: "theme.color.blue" },
  { id: "purple", value: "#8b5cf6", labelKey: "theme.color.purple" }
] as const;

/** Parameters offered by the official rumahl theme (colour, wallpaper, glass). */
const officialParameters: readonly ThemeParameter[] = [
  { kind: "color", id: "accent", token: "color.accent", labelKey: "theme.parameter.accent", seed: true, presets: COLOR_PRESETS },
  { kind: "color", id: "panel", token: "color.surface.strong", labelKey: "theme.parameter.panel" },
  { kind: "color", id: "text", token: "color.on.wallpaper", labelKey: "theme.parameter.text" },
  { kind: "choice", id: "wallpaper", token: "texture.wallpaper", labelKey: "theme.parameter.wallpaper", options: WALLPAPERS },
  {
    kind: "range", id: "morphism", token: "material.morphism", labelKey: "theme.parameter.morphism", min: 0, max: 1, step: 0.05,
    format: (value) => value.toFixed(2),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 1;
    }
  },
  {
    kind: "range", id: "opacity", token: "material.opacity", labelKey: "theme.parameter.opacity", min: 0.2, max: 1, step: 0.02,
    format: (value) => value.toFixed(2),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 0.7;
    }
  },
  px("material.blur", "blur", "theme.parameter.blur", 60, 120),
  {
    kind: "range", id: "saturation", token: "material.saturation", labelKey: "theme.parameter.saturation", min: 1, max: 3, step: 0.1,
    format: (value) => value.toFixed(1),
    parse: (value) => {
      const parsed = Number.parseFloat(value);
      return Number.isFinite(parsed) ? parsed : 1.8;
    }
  },
  px("shape.radius.dock", "radius", "theme.parameter.radius", 24, 40),
  px("shape.radius.window", "window", "theme.parameter.window", 12, 32),
  px("shape.radius.icon", "icon", "theme.parameter.icon", 15, 30)
];

/** The classic fixture offers its own, different set of parameters. */
const classicParameters: readonly ThemeParameter[] = [
  { kind: "color", id: "accent", token: "color.accent", labelKey: "theme.parameter.accent", seed: true },
  px("shape.radius.icon", "icon", "theme.parameter.icon", 6, 20),
  px("shape.radius.window", "window", "theme.parameter.window", 4, 24)
];

/** The rumahl brand default: green accent, glass materials, dock + springboard. */
export const rumahlTheme: Theme = {
  id: "com.rumahl.default",
  name: "rumahl",
  tokens: { ...defaultTokens, ...paletteTokens(defaultTokens["color.accent"]) },
  variants: defaultVariants,
  parameters: officialParameters,
  modes: ["light", "dark"]
};

/**
 * Deliberately different fixture theme: an opaque, square classic look with a
 * taskbar and an app drawer. It exists to prove that a theme can swap the whole
 * shell structure and skin without touching component code.
 */
export const classicTheme: Theme = {
  id: "com.rumahl.classic",
  name: "Classic",
  tokens: {
    ...defaultTokens,
    "color.accent": CLASSIC_COLORS.accent,
    "color.accent.strong": CLASSIC_COLORS.accentStrong,
    "color.surface": "#ececec",
    "color.surface.strong": "#f4f4f4",
    "color.outline": "rgba(0, 0, 0, 0.28)",
    "color.on.wallpaper": "#ffffff",
    "color.on.accent": CLASSIC_COLORS.onAccent,
    "color.shadow": "rgba(0, 0, 0, 0.4)",
    "material.blur": "0px",
    "material.saturation": "1",
    "material.opacity": "1",
    "material.morphism": "0",
    "shape.radius.window": "4px",
    "shape.radius.dock": "6px",
    "shape.radius.icon": "6px",
    "shape.icon.size": "40px",
    "shape.icon.size.large": "56px",
    "motion.duration": "90ms",
    "motion.duration.fast": "70ms",
    "motion.easing.spring": "ease-out",
    "layout.dock.offset": "0px",
    "icon.gradient": "linear-gradient(160deg, #5b8ac6, #3465a4)",
    "texture.wallpaper": "none",
    "typography.family": "Tahoma, Verdana, sans-serif",
    "typography.scale": "0.95"
  },
  variants: { shellLayout: "taskbar", launcherLayout: "drawer", windowChrome: "compact" },
  parameters: classicParameters,
  modes: ["light", "dark"]
};

export const themes = [rumahlTheme, classicTheme] as const;

export const defaultTheme = rumahlTheme;
export const DEFAULT_THEME_ID = rumahlTheme.id;
