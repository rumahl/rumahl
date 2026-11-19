import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlDialog, RumahlModal } from "@rumahl/ui/components";

const meta = {
  title: "Components/Dialog",
  component: RumahlDialog,
  args: {
    open: true,
    onClose: () => {},
    onConfirm: () => {},
    title: "Delete backup?",
    description: "This cannot be undone.",
    confirmLabel: "Delete",
    cancelLabel: "Cancel",
    destructive: true
  },
  argTypes: {
    title: { control: "text" },
    description: { control: "text" },
    confirmLabel: { control: "text" },
    cancelLabel: { control: "text" },
    destructive: { control: "boolean" }
  },
  parameters: {
    docs: {
      description: {
        component: `A modal focused on a single decision.

## Rules
- **One decision per dialog** — the title states it, the actions are explicit.
- \`destructive\` styles the confirm action as danger (for deletions).
- Always provide a cancel / Escape path and trap focus.`
      }
    }
  }
} satisfies Meta<typeof RumahlDialog>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Alert: Story = {
  render: (args) => {
    const [open, setOpen] = useState(true);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Open alert</Button>
        <RumahlDialog
          {...args}
          open={open}
          onClose={() => setOpen(false)}
          onConfirm={() => setOpen(false)}
        />
      </>
    );
  }
};

export const Window: Story = {
  render: () => {
    const [open, setOpen] = useState(true);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Open window</Button>
        <RumahlModal
          open={open}
          onClose={() => setOpen(false)}
          title="System settings"
          application="Settings"
          description="Changes apply immediately."
          closeLabel="Close"
          actions={<Button variant="primary" onClick={() => setOpen(false)}>Save</Button>}
        >
          <p>Modal body content.</p>
        </RumahlModal>
      </>
    );
  }
};
