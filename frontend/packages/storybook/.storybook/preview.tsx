import { useEffect } from "react";
import type { Preview } from "@storybook/react-vite";
import { defaultTheme, ThemeProvider } from "@rumahl/ui";
import { I18nProvider } from "@rumahl/ui/i18n";
import { paletteTokens } from "@rumahl/ui/palette";
import { defineTheme, registerTheme, type ThemeInput } from "@rumahl/ui/sdk";
import pixelRetroData from "../themes/pixel-retro.json";
import referenceData from "../themes/reference.json";
import terminalData from "../themes/terminal.json";
import coreData from "../themes/core.json";

// Installed as pure data: these JSON files are the theme artifacts (tokens +
// stylesheet). No theme values live in this source file — exactly like the OS
// installer, they are defined + registered at load.
const INSTALLED_THEMES = [pixelRetroData, referenceData, terminalData, coreData].map((data) =>
  registerTheme(defineTheme(data as unknown as ThemeInput))
);
// URL-safe toolbar key (Storybook drops global values that contain dots).
const themeKey = (id: string) => id.split(".").pop() ?? id;
const INSTALLED_BY_KEY = new Map(INSTALLED_THEMES.map((theme) => [themeKey(theme.id), theme]));

/** Mirrors the active base onto <html>, like the shell does for portals. */
function ThemeBaseAttribute({ base }: { base: string }) {
  useEffect(() => {
    document.documentElement.dataset.themeBase = base;
    return () => { delete document.documentElement.dataset.themeBase; };
  }, [base]);
  return null;
}
import * as UIComponents from "@rumahl/ui/components";
import * as UIHooks from "@rumahl/ui/hooks";
import * as UIIcons from "@rumahl/ui/icons";
import "./preview.css";
import "@rumahl/ui/materials.css";
import "@rumahl/ui/recipes.css";
import "@rumahl/ui/base.css";
import "../../shell/src/shell/base-ground.css";
import "@rumahl/ui/default-theme.css";
import "@rumahl/ui/components/controls.css";
import "@rumahl/ui/components/scrollbars.css";
import "@rumahl/ui/primitives.css";
import "@rumahl/ui/interactive.css";
import "@rumahl/ui/settings.css";
import "@rumahl/ui/group.css";
import "@rumahl/ui/command.css";
import "@rumahl/ui/glass.css";
import "@rumahl/ui/menubar.css";
import "@rumahl/ui/phone.css";
import "@rumahl/ui/nav.css";
import "@rumahl/ui/chrome.css";

type Scheme = "light" | "dark";

const COMPONENT_NAMES = Object.keys(UIComponents).filter((name) => /^[A-Z]/.test(name));
const HOOK_NAMES = Object.keys(UIHooks);
const ICON_NAMES = Object.keys(UIIcons).filter((name) => /^[A-Z]/.test(name));
const REACT_NAMES = [
  "useState", "useEffect", "useRef", "useMemo", "useCallback", "useId", "useReducer",
  "useContext", "useLayoutEffect", "useSyncExternalStore", "useImperativeHandle",
  "useTransition", "useDeferredValue", "forwardRef", "memo", "createContext",
  "Children", "Fragment", "cloneElement"
];

/**
 * Storybook’s autodocs “Show code” snippet is generated from the story and
 * omits imports. This prepends the imports a reader actually needs, scanning
 * the snippet for which `@rumahl/ui/components` and `@rumahl/ui/hooks` symbols
 * are used, so the copied example runs as-is.
 */
function wrapInFunction(body: string): string {
  const inner = body.trim().replace(/;\s*$/, "");
  const indented = inner
    .split("\n")
    .map((line) => (line.trim() ? `  ${line}` : line))
    .join("\n");
  return `function Story() {\n${indented}\n}`;
}

/** Turn a Storybook `render` story into a readable example. */
function cleanSource(code: string): string {
  let src = code.trim();
  const obj = src.match(/^\{\s*render\s*:\s*([\s\S]*?)\s*,?\s*\};?$/);
  if (obj) src = obj[1].trim();
  const body = src.match(/^(?:\([^()]*\)|[\w$]+)\s*=>\s*\{([\s\S]*)\};?$/);
  if (body) return wrapInFunction(body[1]);
  const expr = src.match(/^(?:\([^()]*\)|[\w$]+)\s*=>\s*([\s\S]+?);?$/);
  if (expr) return wrapInFunction(`return (${expr[1].trim()});`);
  return src;
}

/**
 * Storybook’s autodocs “Show code” is generated from the story and omits
 * imports. This rewrites the story into a readable example and prepends the
 * imports it needs. It is idempotent, so the snippet is never doubled.
 */
function withImports(code: string): string {
  if (/from "@rumahl\/ui\//.test(code) || code.trimStart().startsWith("function Story(")) return code;
  const cleaned = cleanSource(code);
  const jsx = (name: string) => new RegExp(`<${name}(\\s|/|>)`).test(cleaned);
  const components = COMPONENT_NAMES.filter(jsx).sort();
  const icons = ICON_NAMES.filter(jsx).sort();
  const hooks = HOOK_NAMES.filter((name) => new RegExp(`\\b${name}\\(`).test(cleaned)).sort();
  const react = REACT_NAMES.filter((name) => new RegExp(`\\b${name}\\b`).test(cleaned)).sort();
  const lines: string[] = [];
  if (react.length) lines.push(`import { ${react.join(", ")} } from "react";`);
  if (components.length) lines.push(`import { ${components.join(", ")} } from "@rumahl/ui/components";`);
  if (icons.length) lines.push(`import { ${icons.join(", ")} } from "@rumahl/ui/icons";`);
  if (hooks.length) lines.push(`import { ${hooks.join(", ")} } from "@rumahl/ui/hooks";`);
  return lines.length ? `${lines.join("\n")}\n\n${cleaned}` : cleaned;
}

/**
 * Every story renders inside the rumahl theme and the shell's material/control
 * styles, so components look exactly as they do in the OS. The toolbar switches
 * the appearance; dark mode derives its tokens from the same palette engine the
 * OS uses, so custom themes behave identically.
 */
/** Primary/accent colour presets for the toolbar. */
const ACCENTS: Record<string, string> = {
  default: defaultTheme.tokens["color.accent"],
  blue: "#0b84ff",
  mint: "#22c7a9",
  rose: "#f45b85",
  amber: "#ff9f0a",
  violet: "#8b5cf6"
};

/**
 * Bundled OS wallpapers (same assets and dominant tones the shell uses in
 * `wallpapers.ts`). The tone drives the surface tint — exactly like the OS,
 * where glass colours come from the desktop picture and the primary colour.
 */
const WALLPAPERS: Record<string, { url: string; tone: string }> = {
  monstera: { url: "/wallpapers/monstera.jpg", tone: "#0c1619" },
  rodenn: { url: "/wallpapers/wallpaper-john-rodenn.jpg", tone: "#202225" },
  towner: { url: "/wallpapers/wallpaper-john-towner.jpg", tone: "#202225" },
  jonny: { url: "/wallpapers/wallpaper-jonny-james.jpg", tone: "#202225" }
};

/** The wallpaper tint the OS derives: dominant tone at 10% alpha. */
function wallpaperTint(tone: string): string {
  return `${tone}1a`;
}

/** Tokens for the chosen appearance; `accent` re-seeds the primary colour. */
function tokensFor(scheme: Scheme, accent?: string) {
  const seed = (accent && ACCENTS[accent]) || defaultTheme.tokens["color.accent"];
  return { ...defaultTheme.tokens, ...paletteTokens(seed, { mode: scheme }) };
}

const preview: Preview = {
  // Autodocs for every component story, built from the component's JSDoc and
  // props (react-docgen). Foundations/Guides stories keep their own docs.
  tags: ["autodocs"],
  globalTypes: {
    scheme: {
      description: "Appearance",
      toolbar: {
        title: "Appearance",
        icon: "contrast",
        items: [
          { value: "light", title: "Light", icon: "sun" },
          { value: "dark", title: "Dark", icon: "moon" }
        ],
        dynamicTitle: true
      }
    },
    fill: {
      description: "Inner control fill (independent of material)",
      toolbar: {
        title: "Fill",
        icon: "photo",
        items: [
          { value: "solid", title: "Solid fill" },
          { value: "fluid", title: "Fluid fill" }
        ],
        dynamicTitle: true
      }
    },
    material: {
      description: "Surface material",
      toolbar: {
        title: "Material",
        icon: "mirror",
        items: [
          { value: "glass", title: "Glass", icon: "mirror" },
          { value: "floating", title: "Floating (liquid glass)", icon: "circle" },
          { value: "solid", title: "Solid", icon: "box" }
        ],
        dynamicTitle: true
      }
    },
    accent: {
      description: "Primary colour",
      toolbar: {
        title: "Primary",
        icon: "paintbrush",
        items: [
          { value: "default", title: "Default" },
          { value: "blue", title: "Blue" },
          { value: "mint", title: "Mint" },
          { value: "rose", title: "Rose" },
          { value: "amber", title: "Amber" },
          { value: "violet", title: "Violet" }
        ],
        dynamicTitle: true
      }
    },
    theme: {
      description: "Design theme",
      toolbar: {
        title: "Theme",
        icon: "paintbrush",
        items: [
          { value: "default", title: "Iris" },
          ...INSTALLED_THEMES.map((theme) => ({ value: themeKey(theme.id), title: theme.name }))
        ],
        dynamicTitle: true
      }
    },
    device: {
      description: "Viewport width",
      toolbar: {
        title: "Device",
        icon: "browser",
        items: [
          { value: "desktop", title: "Desktop" },
          { value: "tablet", title: "Tablet · 820" },
          { value: "phone", title: "Phone · 390" }
        ],
        dynamicTitle: true
      }
    },
    background: {
      description: "Surface the components are shown on",
      toolbar: {
        title: "Surface",
        icon: "photo",
        items: [
          { value: "window", title: "Window (panel)" },
          { value: "canvas", title: "Canvas" },
          { value: "none", title: "None" }
        ],
        dynamicTitle: true
      }
    },
    wallpaper: {
      description: "Desktop picture behind translucent surfaces",
      toolbar: {
        title: "Wallpaper",
        icon: "photo",
        items: [
          { value: "monstera", title: "Monstera" },
          { value: "rodenn", title: "Roden" },
          { value: "towner", title: "Towner" },
          { value: "jonny", title: "Jonny" }
        ],
        dynamicTitle: true
      }
    }
  },
  initialGlobals: {
    scheme: "light",
    material: "glass",
    fill: "solid",
    accent: "default",
    theme: "default",
    device: "desktop",
    background: "window",
    wallpaper: "monstera"
  },
  decorators: [
    (Story, context) => {
      const installedTheme = INSTALLED_BY_KEY.get(context.globals.theme as string);
      const scheme: Scheme = installedTheme
        ? installedTheme.modes.includes("light")
          ? "light"
          : "dark"
        : context.globals.scheme === "dark"
          ? "dark"
          : "light";
      const material = installedTheme
        ? installedTheme.material ?? "solid"
        : ["solid", "floating"].includes(context.globals.material)
          ? context.globals.material
          : "glass";
      const width =
        context.globals.device === "phone" ? 390 : context.globals.device === "tablet" ? 820 : undefined;
      const bg = context.globals.background;
      const isWindow = bg === "window";
      const inDocs = context.viewMode === "docs";
      const wp = WALLPAPERS[context.globals.wallpaper as string] ?? WALLPAPERS.monstera;
      // Whole-app stories (Layout/*) are applications: they bring their own
      // background by covering the whole surface with the app window material
      // (no wallpaper showing through), edge to edge. Individual components
      // that ship their own surface (a window frame, a toast, a popup, …) opt
      // out via `parameters.windowSurface = false`.
      const isLayout =
        typeof context.title === "string" && context.title.startsWith("Layout/");
      const withAppSurface =
        isWindow && context.parameters.windowSurface !== false;
      const edgeToEdge = isLayout;
      // Layers, bottom to top: desktop picture → app window background → the
      // components. The picture stays visible through the glass. In docs the
      // stack is a contained block (a fixed element would be clipped and overlap
      // the next story); in story view it fills the viewport.
      const background = isWindow
        ? `${wp.tone} url("${wp.url}") center / cover no-repeat`
        : bg === "none"
          ? "transparent"
          : "var(--rumahl-ui-color-canvas-background)";
      // The OS tints surfaces with the desktop picture's dominant colour; mirror
      // that here so the glass colour is derived from wallpaper + primary colour,
      // not a fixed recipe.
      const theme = installedTheme ?? {
        ...defaultTheme,
        tokens: {
          ...tokensFor(scheme, context.globals.accent),
          "color.wallpaper.tint": wallpaperTint(wp.tone)
        }
      };
      return (
        <ThemeProvider theme={theme}>
          <ThemeBaseAttribute base={theme.base ?? "iris"} />
          <I18nProvider locale="en">
            {installedTheme?.css ? (
              <style dangerouslySetInnerHTML={{ __html: installedTheme.css }} />
            ) : null}
            <div
              data-material={material}
              style={{
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                ...(inDocs
                  ? {
                      position: "relative",
                      minHeight: 340,
                      overflow: "hidden",
                      padding: edgeToEdge ? 0 : 24,
                      borderRadius: edgeToEdge ? 0 : 16
                    }
                  : {
                      position: "fixed",
                      inset: 0,
                      overflow: "auto",
                      padding: edgeToEdge ? 0 : width ? 12 : 24
                    }),
                background
              }}
            >
              {withAppSurface ? (
                <div
                  aria-hidden="true"
                  className="rumahl-app-surface"
                  style={{
                    position: "absolute",
                    inset: 0,
                    pointerEvents: "none",
                    // Apps that don't need the desktop picture cover it with an
                    // opaque background instead of the translucent glass.
                    ...(edgeToEdge
                      ? {
                          background: "var(--rumahl-ui-color-window-background-solid)",
                          backdropFilter: "none",
                          WebkitBackdropFilter: "none"
                        }
                      : {})
                  }}
                />
              ) : null}
              <div
                className="shell"
                data-theme={theme.id}
                data-theme-base={theme.base ?? "iris"}
                data-scheme={scheme}
                data-material={material}
                data-fill={context.globals.fill === "fluid" ? "fluid" : "solid"}
                style={{
                  position: "relative",
                  padding: isLayout ? 0 : 30,
                  ...(isLayout
                    ? { width: "100%" }
                    : withAppSurface
                      ? {
                          // Full-width row that wraps: small components centre,
                          // full-width ones (Steps, tables, …) take their own line
                          // instead of being squeezed to content width.
                          width: "100%",
                          display: "flex",
                          flexWrap: "wrap",
                          justifyContent: "center",
                          alignItems: "center",
                          textAlign: "center"
                        }
                      : {}),
                  ...(width ? { width, minHeight: 720 } : {})
                }}
              >
                <Story />
              </div>
            </div>
          </I18nProvider>
        </ThemeProvider>
      );
    }
  ],
  parameters: {
    options: {
      storySort: {
        order: [
          "Welcome",
          "Catalog",
          "Design System",
          "Foundations",
          "Components",
          "Primitives",
          "System",
          "Layout",
          "Patterns",
          "Hooks",
          "Guides"
        ]
      }
    },
    layout: "centered",
    controls: { matchers: { color: /(background|color)$/i, date: /Date$/i } },
    // Accessibility (incl. colour contrast) runs on every story and fails the
    // test-runner on violations, so contrast regressions are caught reliably.
    a11y: {
      test: "error",
      config: {
        rules: [
          { id: "color-contrast", enabled: true },
          { id: "aria-allowed-attr", enabled: true }
        ]
      }
    },
    docs: { source: { language: "tsx", transform: withImports } }
  }
};

export default preview;
