import type { Meta, StoryObj } from "@storybook/react-vite";

/**
 * # Elevation
 *
 * Depth comes from a few shared recipes (CSS custom properties) rather than
 * one-off shadows, so glass, floating and solid materials stay consistent.
 *
 * - `--control-inset` — recessed controls (inputs, buttons at rest)
 * - `--control-button-shadow` — raised surfaces (active toggles, thumbs)
 * - `--surface-elevation` — floating overlays (glass material)
 * - `--control-popup-shadow` — popovers and menus
 *
 * Switch the Material toolbar to see the same elevation on different materials.
 */
const meta = {
  title: "Foundations/Elevation",
  tags: ["autodocs"],
  parameters: { layout: "centered" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const RECIPES = [
  { name: "--control-inset", css: "var(--control-inset)" },
  { name: "--control-button-shadow", css: "var(--control-button-shadow)" },
  { name: "--control-popup-shadow", css: "var(--control-popup-shadow)" }
];

export const Recipes: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 28, flexWrap: "wrap", padding: 40 }}>
      {RECIPES.map((recipe) => (
        <div key={recipe.name} style={{ display: "grid", gap: 10, width: 160 }}>
          <div
            style={{
              height: 88,
              borderRadius: 14,
              background: "var(--rumahl-ui-color-surface-strong)",
              boxShadow: recipe.css
            }}
          />
          <div style={{ fontSize: 12}}>{recipe.name}</div>
        </div>
      ))}
    </div>
  )
};
