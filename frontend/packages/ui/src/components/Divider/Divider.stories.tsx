import type { Meta, StoryObj } from "@storybook/react-vite";
import { Divider, Flex, Text } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Divider",
  component: Divider,
  args: { orientation: "horizontal" },
  argTypes: { orientation: { control: "inline-radio", options: ["horizontal", "vertical"] } },
  parameters: {
    docs: {
      description: {
        component: `A hairline separator (horizontal or vertical).

## Rules
- Use to separate **groups**, not as decoration.
- Prefer \`gap\` / grouping over dividers when you can.`
      }
    }
  }
} satisfies Meta<typeof Divider>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <div style={{ width: 260 }}>
      <Text>Above</Text>
      <Divider {...args} style={{ margin: "8px 0" }} />
      <Text>Below</Text>
    </div>
  )
};

export const Vertical: Story = {
  render: () => (
    <Flex align="center" gap={3} style={{ height: 32 }}>
      <Text>Left</Text>
      <Divider orientation="vertical" />
      <Text>Right</Text>
    </Flex>
  )
};
