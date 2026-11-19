import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMediaQuery } from "@rumahl/ui/hooks";
import { Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useMediaQuery", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const wide = useMediaQuery("(min-width: 700px)");
    const prefersReduced = useMediaQuery("(prefers-reduced-motion: reduce)");
    return (
      <div style={{ display: "grid", gap: 6 }}>
        <Text>min-width: 700px → {wide ? "true" : "false"}</Text>
        <Text>prefers-reduced-motion: reduce → {prefersReduced ? "true" : "false"}</Text>
      </div>
    );
  }
};
