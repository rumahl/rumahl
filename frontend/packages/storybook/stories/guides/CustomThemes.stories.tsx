import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";
import { listThemes } from "@rumahl/ui/sdk";

/**
 * # Custom themes
 *
 * A theme is a plain object. Author it with `defineTheme`, publish it with
 * `registerTheme`, and the shell resolves it by `id` — no component changes.
 *
 * ## Full example
 *
 * ```ts
 * import { defineTheme, registerTheme } from "@rumahl/ui/sdk";
 *
 * export const forest = registerTheme(defineTheme({
 *   id: "com.example.forest",
 *   name: "Forest",
 *   material: "glass",            // "glass" | "solid" (whole-shell default)
 *   modes: ["light", "dark"],
 *   tokens: {
 *     "color.accent": "#2f8f5b",
 *     "color.accent.strong": "#57b47f",
 *     "color.canvas.background": "#0e1a14",
 *     "color.panel.background": "#16241d",
 *     "color.surface.strong": "#1d2f26",
 *     "color.text.primary": "#e9f2ec",
 *     "color.text.muted": "#9db3a7",
 *     "color.outline": "rgba(255,255,255,0.14)",
 *     "shape.radius.window": "14px",
 *     "material.opacity": "0.72",
 *     "typography.family": "system-ui, sans-serif",
 *   },
 *   // Optional theme CSS (presentation only; applied CSP-safely).
 *   css: ".shell { --brand: #2f8f5b; }",
 * }));
 * ```
 *
 * ## Dark and light
 * `modes: ["light", "dark"]` enables the appearance switch. Derive per-scheme
 * colours from a seed with `paletteTokens(seed, { mode })` instead of listing
 * every role by hand.
 *
 * ```ts
 * import { paletteTokens } from "@rumahl/ui/palette";
 * const tokens = { ...defaultTokens, ...paletteTokens("#2f8f5b", { mode: "dark" }) };
 * ```
 *
 * ## Validation and error cases
 * - `defineTheme` throws when `id` or `name` is empty.
 * - Omitted tokens fall back to `defaultTokens`, so a theme is always complete.
 * - `parameters` must reference a real token id; colour/ranges are clamped.
 * - When the platform receives a theme manifest, unknown tokens and unsafe
 *   values are rejected — a theme can never inject code.
 *
 * ## Registering / resolving
 * - `registerTheme(theme)` adds it to the registry; `listThemes()` returns all.
 * - The shell picks the active theme from the account + device preferences and
 *   wraps the app in `ThemeProvider`.
 */
const meta = {
  title: "Guides/Custom Themes",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component:
          "A complete custom-theme walkthrough: authoring, dark/light, " +
          "validation, registration and resolution."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Registry() {
  const { theme } = useTheme();
  return (
    <div style={{ display: "grid", gap: 8, fontSize: 13}}>
      <div>
        Active: <strong>{theme.name}</strong> <code>{theme.id}</code>
      </div>
      <div style={{ color: "var(--rumahl-ui-color-text-muted)" }}>
        Registered themes: {listThemes().map((item) => `${item.name} (${item.id})`).join(", ")}
      </div>
      <div style={{ color: "var(--rumahl-ui-color-text-muted)" }}>
        Modes: {theme.modes.join(", ")} · material: {theme.material ?? "glass"}
      </div>
    </div>
  );
}

export const Registry_: Story = {
  name: "Registry",
  render: () => <Registry />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { defineTheme, registerTheme, listThemes } from "@rumahl/ui/sdk";

registerTheme(
  defineTheme({
    id: "com.example.ocean",
    name: "Ocean",
    tokens: {
      "color.accent": "#0b6ea8",
      "color.canvas.background": "#071a24",
      "color.panel.background": "#0d2733"
    },
    modes: ["light", "dark"]
  })
);

listThemes(); // -> [{ id: "com.example.ocean", name: "Ocean" }, …]`
      }
    }
  }
};
