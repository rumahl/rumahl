import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useKeyEvent } from "@rumahl/ui/hooks";
import { Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useKeyEvent", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [count, setCount] = useState(0);
    useKeyEvent("k", () => setCount((value) => value + 1));
    return <Text>Press `k` anywhere (pressed {count}×).</Text>;
  }
};
