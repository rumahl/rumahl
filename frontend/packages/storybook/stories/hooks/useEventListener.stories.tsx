import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEventListener } from "@rumahl/ui/hooks";
import { Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useEventListener", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [width, setWidth] = useState(typeof window === "undefined" ? 0 : window.innerWidth);
    useEventListener(typeof window === "undefined" ? null : window, "resize", () =>
      setWidth(window.innerWidth)
    );
    return <Text>Window width: {width}px — resize to update.</Text>;
  }
};
