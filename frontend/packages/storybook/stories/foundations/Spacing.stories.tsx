import type { Meta, StoryObj } from "@storybook/react-vite";

/**
 * # Spacing
 *
 * One 4px base scale powers layout (`Box`/`Flex` `p`/`m`/`gap` props) and the
 * shared control size scale (`data-size` → `--control-h/-pad-x/-pad-y`).
 *
 * | Step | px | Typical use |
 * |---|---|---|
 * | 1 | 4 | tight icon gaps |
 * | 2 | 8 | inline gaps |
 * | 3 | 12 | control padding |
 * | 4 | 16 | card padding |
 * | 6 | 24 | section padding |
 * | 8 | 32 | page gutters |
 *
 * ## Control size scale
 *
 * | `size` | control height |
 * |---|---|
 * | xs | 26px |
 * | sm | 30px |
 * | md | 36px (default) |
 * | lg | 44px |
 * | xl | 52px |
 */
const meta = {
  title: "Foundations/Spacing",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const SPACES = [1, 2, 3, 4, 6, 8];

export const SpacingScale: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 14, padding: 24 }}>
      {SPACES.map((step) => (
        <div key={step} style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span style={{ width: 48, fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>
            {step} · {step * 4}px
          </span>
          <span
            style={{
              width: step * 4 * 4,
              height: 16,
              borderRadius: 4,
              background: "var(--rumahl-ui-color-accent)"
            }}
          />
        </div>
      ))}
    </div>
  )
};
