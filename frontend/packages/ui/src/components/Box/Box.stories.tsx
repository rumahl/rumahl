import type { Meta, StoryObj } from "@storybook/react-vite";
import { Box, Text } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Box",
  component: Box,
  args: { p: 4, radius: "12px", background: "var(--rumahl-ui-color-surface-strong)", border: true },
  argTypes: {
    p: { control: { type: "number" } },
    px: { control: { type: "number" } },
    py: { control: { type: "number" } },
    gap: { control: { type: "number" } },
    radius: { control: "text" },
    background: { control: "text" },
    border: { control: "boolean" },
    as: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `The layout primitive: padding, gap, radius, background and border, all
token-based.

## Rules
- Use the **tokens** — \`p\`/\`px\`/\`py\`/\`gap\` are 4px steps, radius a shape token.
- A plain surface; for layout flows prefer **Flex**.`
      }
    }
  }
} satisfies Meta<typeof Box>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <Box {...args}>
      <Text>A padded, bordered surface made from a single Box.</Text>
    </Box>
  )
};
