import type { Meta, StoryObj } from "@storybook/react-vite";
import { useHover } from "@rumahl/ui/hooks";
import { Box, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useHover", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [ref, hovered] = useHover<HTMLDivElement>();
    return (
      <Box
        ref={ref}
        p={5}
        radius="12px"
        border
        background={hovered ? "var(--rumahl-ui-color-accent)" : "var(--rumahl-ui-color-surface-strong)"}
      >
        <Text tone={hovered ? "default" : "default"}>Hovered: {hovered ? "yes" : "no"}</Text>
      </Box>
    );
  }
};
