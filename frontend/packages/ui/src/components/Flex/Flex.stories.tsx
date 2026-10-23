import type { Meta, StoryObj } from "@storybook/react-vite";
import { Box, Flex, Text } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Flex",
  component: Flex,
  args: { direction: "row", align: "center", justify: "start", gap: 3 },
  argTypes: {
    direction: { control: "inline-radio", options: ["row", "column", "row-reverse", "column-reverse"] },
    align: { control: "inline-radio", options: ["start", "center", "end", "stretch", "baseline"] },
    justify: { control: "inline-radio", options: ["start", "center", "end", "between", "around", "evenly"] },
    wrap: { control: "boolean" },
    inline: { control: "boolean" },
    gap: { control: { type: "number" } }
  },
  parameters: {
    docs: {
      description: {
        component: `The flexbox primitive (direction / align / justify / gap / wrap).

## Rules
- Use \`gap\` (token steps) instead of margins between children.
- \`inline\` shrink-wraps a row; otherwise Flex fills its parent.`
      }
    }
  }
} satisfies Meta<typeof Flex>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <Flex {...args}>
      {[1, 2, 3].map((n) => (
        <Box key={n} p={3} radius="8px" background="var(--rumahl-ui-color-surface-strong)" border>
          <Text>Item {n}</Text>
        </Box>
      ))}
    </Flex>
  )
};
