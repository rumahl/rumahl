import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlMark } from "@rumahl/ui/components";

const meta = {
  title: "Components/Mark",
  component: RumahlMark,
  parameters: {
    docs: {
      description: {
        component: `The rumahl brand mark.

- \`variant="solid"\` (default) — filled with \`currentColor\`.
- \`variant="glass"\` — cut into the surface (debossed) with a tinted fill,
  matching the recessed glass controls.
- \`variant="engraved"\` — the mark itself with **no fill colour**: only the cut
  edge, so the surface shows through.

## Rules
- Use it as-is; keep clear space around it.
- Don't recolour beyond the token, or distort its proportions.
- Use the \`glass\` variant only **on a glass surface**, never floating over an
  image.`
      }
    }
  }
} satisfies Meta<typeof RumahlMark>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = { render: () => <RumahlMark style={{ width: 40, height: 72 }} /> };
export const Tinted: Story = {
  render: () => <RumahlMark style={{ width: 40, height: 72, color: "#22c7a9" }} />
};
export const Sizes: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "flex-end", gap: 16 }}>
      {[16, 28, 48, 80].map((size) => (
        <RumahlMark key={size} style={{ width: size * 0.53, height: size }} />
      ))}
    </div>
  )
};

/** `variant="glass"` — the mark looks cut into the recessed glass surface. */
export const CutIntoGlass: Story = {
  name: "Cut into glass",
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: 28 }}>
      <RumahlMark variant="glass" style={{ width: 40, height: 76 }} />
      <RumahlMark variant="glass" style={{ width: 40, height: 76, color: "#22c7a9" }} />
      <RumahlMark variant="engraved" style={{ width: 40, height: 76 }} />
      <RumahlMark variant="engraved" style={{ width: 26, height: 50, color: "#22c7a9" }} />
    </div>
  )
};
