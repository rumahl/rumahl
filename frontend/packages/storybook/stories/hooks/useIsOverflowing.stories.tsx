import type { Meta, StoryObj } from "@storybook/react-vite";
import { useIsOverflowing } from "@rumahl/ui/hooks";
import { Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useIsOverflowing", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [ref, overflowing] = useIsOverflowing<HTMLDivElement>();
    return (
      <div style={{ width: 220 }}>
        <div ref={ref} style={{ overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis" }}>
          <Text>A fairly long label that may or may not overflow its container box.</Text>
        </div>
        <Text tone="muted">overflowing: {overflowing ? "true" : "false"}</Text>
      </div>
    );
  }
};
