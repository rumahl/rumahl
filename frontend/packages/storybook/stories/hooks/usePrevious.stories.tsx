import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { usePrevious } from "@rumahl/ui/hooks";
import { Button, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/usePrevious", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [count, setCount] = useState(0);
    const previous = usePrevious(count);
    return (
      <div style={{ display: "grid", gap: 10, justifyItems: "start" }}>
        <Text>current: {count}</Text>
        <Text tone="muted">previous: {previous ?? "—"}</Text>
        <Button onClick={() => setCount((value) => value + 1)}>Increment</Button>
      </div>
    );
  }
};
