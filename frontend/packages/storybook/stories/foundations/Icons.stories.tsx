import type { Meta, StoryObj } from "@storybook/react-vite";
import * as Icons from "@rumahl/ui/icons";
import type { ComponentType, SVGProps } from "react";

/**
 * # Icons
 *
 * The full rumahl icon set (`@rumahl/ui/icons`). Line icons on a 24px grid,
 * `stroke: currentColor`, 1.7px stroke; they inherit colour from the text.
 *
 * ## Rules
 *
 * - **Icons are decorative by default** (`aria-hidden`). The control carries the
 *   accessible name (`label` / `aria-label`), never the icon.
 * - **Size follows the control** (`--control-icon` / the size scale); do not
 *   hand-set icon pixels inside a control.
 * - **Never let an icon carry meaning alone** — pair it with text or an
 *   accessible name.
 * - **Colour is inherited** via `currentColor`; do not hard-code an icon colour.
 */
const meta = {
  title: "Foundations/Icons",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { description: { component: "Every icon exported by `@rumahl/ui/icons`." } }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const NAMES = Object.keys(Icons)
  .filter((name) => /^[A-Z]/.test(name))
  .sort();

export const Gallery: Story = {
  render: () => (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))",
        gap: 14,
        padding: 24
      }}
    >
      {NAMES.map((name) => {
        const Icon = (Icons as Record<string, ComponentType<SVGProps<SVGSVGElement>>>)[name];
        if (!Icon) return null;
        return (
          <div
            key={name}
            style={{
              display: "grid",
              justifyItems: "center",
              gap: 8,
              padding: 14,
              border: "1px solid var(--surface-edge)",
              borderRadius: 12,
              background: "var(--surface-sheen), var(--surface-fill)"
            }}
          >
            <span style={{ display: "inline-flex", width: 22, height: 22, color: "var(--rumahl-ui-color-text-primary)" }}>
              <Icon />
            </span>
            <code style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)", textAlign: "center", overflowWrap: "anywhere" }}>
              {name}
            </code>
          </div>
        );
      })}
    </div>
  )
};
