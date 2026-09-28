import { defaultTokens, type Tokens } from "./tokens";
import { defaultVariants, type VariantSelection } from "./variants";

/**
 * A resolved theme is plain data: token values plus the variant selection.
 * It can safely cross the wire (validated by the platform) or be authored by
 * developers with `defineTheme`.
 */
export interface Theme {
  id: string;
  name: string;
  tokens: Tokens;
  variants: VariantSelection;
}

/** The rumahl brand default: green accent, glass materials, dock + springboard. */
export const rumahlTheme: Theme = {
  id: "com.rumahl.default",
  name: "rumahl",
  tokens: defaultTokens,
  variants: defaultVariants
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
    "color.accent": "#3465a4",
    "color.accent-strong": "#5b8ac6",
    "color.surface": "#ececec",
    "color.surface-strong": "#f4f4f4",
    "color.outline": "rgba(0, 0, 0, 0.28)",
    "color.on-wallpaper": "#ffffff",
    "color.on-accent": "#ffffff",
    "color.shadow": "rgba(0, 0, 0, 0.4)",
    "material.blur": "0px",
    "material.saturation": "1",
    "shape.radius-window": "4px",
    "shape.radius-dock": "6px",
    "shape.radius-icon": "6px",
    "shape.icon-size": "40px",
    "shape.icon-size-large": "56px",
    "motion.duration": "90ms",
    "motion.duration-fast": "70ms",
    "motion.easing-spring": "ease-out",
    "layout.dock-offset": "0px",
    "icon.gradient": "linear-gradient(160deg, #5b8ac6, #3465a4)",
    "texture.wallpaper": "none",
    "typography.family": "Tahoma, Verdana, sans-serif",
    "typography.scale": "0.95"
  },
  variants: { shellLayout: "taskbar", launcherLayout: "drawer", windowChrome: "compact" }
};

export const themes = [rumahlTheme, classicTheme] as const;

export const defaultTheme = rumahlTheme;
export const DEFAULT_THEME_ID = rumahlTheme.id;
