import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTimeout } from "@rumahl/ui/hooks";
import { Button, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useTimeout", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [armed, setArmed] = useState(false);
    useTimeout(() => setArmed(false), armed ? 2000 : null);
    return (
      <div style={{ display: "grid", gap: 10, justifyItems: "start" }}>
        <Text>{armed ? "Disarms in 2s…" : "Idle"}</Text>
        <Button onClick={() => setArmed(true)}>Arm</Button>
      </div>
    );
  }
};
