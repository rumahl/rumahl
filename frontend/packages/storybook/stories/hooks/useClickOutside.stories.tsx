import { useRef, useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useClickOutside } from "@rumahl/ui/hooks";
import { Box, Button, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useClickOutside", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [open, setOpen] = useState(false);
    const ref = useRef<HTMLDivElement>(null);
    useClickOutside(ref, () => setOpen(false), open);
    return (
      <div style={{ display: "grid", gap: 12 }}>
        <Button onClick={() => setOpen(true)}>Open panel</Button>
        {open ? (
          <Box ref={ref} p={4} radius="12px" border background="var(--rumahl-ui-color-surface-strong)">
            <Text>Click anywhere outside this box to close it.</Text>
          </Box>
        ) : null}
      </div>
    );
  }
};
