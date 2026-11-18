import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";
import { Button, RumahlInput } from "@rumahl/ui/components";
import { listThemes } from "@rumahl/ui/sdk";

/**
 * # Theming
 *
 * A theme is **data, not code**: a set of tokens (plus optional CSS and safe
 * slot templates) that the shell projects onto the document root as
 * `--rumahl-ui-*` custom properties. Components only read those tokens, so a
 * theme restyles the whole OS without touching component code.
 *
 * ## Create a theme
 *
 * ```ts
 * import { defineTheme, registerTheme } from "@rumahl/ui/sdk";
 *
 * export const ocean = registerTheme(defineTheme({
 *   id: "com.example.ocean",
 *   name: "Ocean",
 *   tokens: {
 *     "color.accent": "#0b6ea8",
 *     "color.canvas.background": "#071a24",
 *     "color.panel.background": "#0d2733",
 *     "color.text.primary": "#eaf4f9",
 *     "shape.radius.window": "16px",
 *   },
 *   modes: ["light", "dark"],
 * }));
 * ```
 *
 * ## Token contract
 * - `color.*` — accent, surfaces, text, canvas, panel, outline, shadow.
 * - `material.*` — blur, saturation, opacity, morphism (glass strength).
 * - `shape.*` — corner radii and icon sizes.
 * - `motion.*` — durations and easing.
 * - `typography.*` — UI and display faces, and scale.
 * - `layout.*`, `texture.wallpaper`, `icon.gradient`.
 *
 * ## Materials (cross-cutting states)
 * - **Glass** (default) — recessed glass for controls.
 * - **Solid** — flat and opaque.
 * - **Floating** — element state for controls that hover over a background.
 *
 * ## Applying a theme
 * The shell resolves the active theme (account + device preferences) and wraps
 * the app in `ThemeProvider`; `applyTokens` projects the tokens onto `:root`.
 * A theme may also ship `css` for theme-specific presentation.
 */
const meta = {
  title: "Guides/Theming",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component:
          "How tokens, materials and `defineTheme` work — the foundation for " +
          "custom themes built on rumahl/ui."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Live() {
  const { theme, tokens } = useTheme();
  const sample = [
    "color.accent",
    "color.canvas.background",
    "color.panel.background",
    "color.text.primary",
    "shape.radius.window",
    "material.opacity"
  ] as const;
  return (
    <div style={{ display: "grid", gap: 16, width: 460 }}>
      <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>
        Active theme: <strong>{theme.name}</strong> · registered: {listThemes().map((t) => t.name).join(", ")}
      </div>
      <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
        <Button variant="primary">Primary</Button>
        <Button>Secondary</Button>
        <RumahlInput placeholder="Input" />
      </div>
      <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "6px 16px", fontSize: 12, margin: 0 }}>
        {sample.map((key) => (
          <div key={key} style={{ display: "contents" }}>
            <dt style={{ color: "var(--rumahl-ui-color-text-muted)" }}>{key}</dt>
            <dd style={{ margin: 0 }}>{(tokens as Record<string, string>)[key]}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export const LiveTokens: Story = {
  render: () => <Live />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { useTheme } from "@rumahl/ui";

export function Palette() {
  const { theme, tokens } = useTheme();
  const keys = [
    "color.accent",
    "color.canvas.background",
    "color.panel.background",
    "color.text.primary",
    "shape.radius.window",
    "material.opacity"
  ];
  return (
    <div style={{ display: "grid", gap: 12, width: 460 }}>
      {keys.map((key) => (
        <div key={key} style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span
            style={{
              width: 22,
              height: 22,
              borderRadius: 6,
              background: tokens[key as keyof typeof tokens] as string
            }}
          />
          <code style={{ flex: 1 }}>{key}</code>
          <span style={{ color: "var(--rumahl-ui-color-text-muted)" }}>
            {tokens[key as keyof typeof tokens]}
          </span>
        </div>
      ))}
    </div>
  );
}`
      }
    }
  }
};
