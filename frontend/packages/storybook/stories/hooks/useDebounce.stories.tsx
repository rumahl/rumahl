import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useDebounce } from "@rumahl/ui/hooks";
import { RumahlInput, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useDebounce", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [value, setValue] = useState("");
    const debounced = useDebounce(value, 500);
    return (
      <div style={{ display: "grid", gap: 8, width: 320 }}>
        <RumahlInput placeholder="Type…" value={value} onChange={(event) => setValue(event.target.value)} />
        <Text>live: {value || "—"}</Text>
        <Text tone="accent">debounced (500ms): {debounced || "—"}</Text>
      </div>
    );
  }
};
