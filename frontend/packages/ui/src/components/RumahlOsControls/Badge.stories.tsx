import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlBadge } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Badge",
  component: RumahlBadge,
  args: { children: "Success", tone: "success" },
  argTypes: {
    ...nativeArgTypes,
    tone: { control: "inline-radio", options: ["neutral", "success", "warning", "danger"] },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A small, **read-only** status pill (a word, a tone, an optional dot).

## Rules
- Badges show **status** — they are not buttons (use a Chip to filter/select).
- Keep to one or two words.
- Tones are semantic (neutral / success / warning / danger) — never decorative.`
      }
    }
  }
} satisfies Meta<typeof RumahlBadge>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Tones: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 8 }}>
      {(["neutral", "success", "warning", "danger"] as const).map((tone) => (
        <RumahlBadge key={tone} tone={tone}>
          {tone}
        </RumahlBadge>
      ))}
    </div>
  )
};

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlBadge key={size} size={size} tone="success">
          Size {size}
        </RumahlBadge>
      ))}
    </div>
  )
};
