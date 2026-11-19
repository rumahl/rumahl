import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Button,
  RumahlBadge,
  RumahlDialog,
  RumahlNotice,
  RumahlProgress
} from "@rumahl/ui/components";

const FEEDBACK_DOCS = `# Feedback

Communicate state with badges (inline status), notices (callouts), progress
(ongoing work) and a dialog (blocking confirmation).

## Do
- Pick the **lightest** surface for the urgency: Badge (inline status) →
  Notice (contextual callout) → Toast (transient event) → Dialog (blocking
  decision).
- Label progress and expose a value; lead a notice with the consequence.
- Use \`destructive\` on the confirming action when it can't be undone.

## Don't
- Don't stack several notices for one message.
- Don't use a Toast for something the user must act on.
- Don't rely on colour alone (badge tone / progress fill) — always add text.`;

const meta = {
  title: "Patterns/Feedback",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: { description: { component: FEEDBACK_DOCS } }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Status: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16, width: 460 }}>
      <div style={{ display: "flex", gap: 8 }}>
        <RumahlBadge>Neutral</RumahlBadge>
        <RumahlBadge tone="success">Up to date</RumahlBadge>
        <RumahlBadge tone="warning">Update available</RumahlBadge>
        <RumahlBadge tone="danger">Failed</RumahlBadge>
      </div>
      <RumahlNotice title="Update available" tone="info">
        A new version is ready to install.
      </RumahlNotice>
      <RumahlNotice title="Low storage" tone="warning">Only 3 GB left.</RumahlNotice>
      <RumahlProgress label="Installing" value={42} />
    </div>
  )
};

function Confirm() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="danger" onClick={() => setOpen(true)}>Delete backup</Button>
      <RumahlDialog
        open={open}
        onClose={() => setOpen(false)}
        onConfirm={() => setOpen(false)}
        title="Delete this backup?"
        description="This cannot be undone."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        destructive
      />
    </>
  );
}

export const Confirmation: Story = {
  render: () => <Confirm />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { useState } from "react";
import { Button, RumahlDialog } from "@rumahl/ui/components";

export function DeleteBackup() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="danger" onClick={() => setOpen(true)}>
        Delete backup
      </Button>
      <RumahlDialog
        open={open}
        onClose={() => setOpen(false)}
        onConfirm={() => setOpen(false)}
        title="Delete this backup?"
        description="This cannot be undone."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        destructive
      />
    </>
  );
}`
      }
    }
  }
};
