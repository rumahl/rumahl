import type { Meta, StoryObj } from "@storybook/react-vite";
import { Text } from "@rumahl/ui/components";

const SIZE = ["xs", "sm", "md", "lg", "xl"] as const;

const meta = {
  title: "Primitives/Text",
  component: Text,
  args: { children: "The quick brown fox jumps over the lazy dog", size: "md", tone: "default" },
  argTypes: {
    size: { control: "inline-radio", options: SIZE },
    tone: { control: "inline-radio", options: ["default", "muted", "accent", "success", "warning", "danger"] },
    weight: { control: "inline-radio", options: ["regular", "medium", "semibold", "bold"] },
    truncate: { control: "boolean" },
    children: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `Body / label text with size, weight, tone and optional truncation.

## Rules
- Use \`tone="muted"\` for secondary text — **never** reduced opacity.
- \`truncate\` only when the full text is available elsewhere.`
      }
    }
  }
} satisfies Meta<typeof Text>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Sizes: Story = {
  render: (args) => (
    <div style={{ display: "grid", gap: 8 }}>
      {SIZE.map((size) => (
        <Text {...args} key={size} size={size}>
          {size} — {args.children}
        </Text>
      ))}
    </div>
  )
};
export const Tones: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 6 }}>
      {(["default", "muted", "accent", "success", "warning", "danger"] as const).map((tone) => (
        <Text key={tone} tone={tone}>{tone}</Text>
      ))}
    </div>
  )
};
export const Truncate: Story = {
  render: () => (
    <div style={{ width: 180 }}>
      <Text truncate>The quick brown fox jumps over the lazy dog</Text>
    </div>
  )
};
