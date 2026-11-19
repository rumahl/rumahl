import type { Meta, StoryObj } from "@storybook/react-vite";
import { useAfterFirstRender } from "@rumahl/ui/hooks";
import { Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useAfterFirstRender", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const after = useAfterFirstRender();
    return <Text>After first render: {after ? "true" : "false"}</Text>;
  }
};
