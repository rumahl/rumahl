import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlEmptyState } from "@rumahl/ui/components";

const meta = {
  title: "Components/Empty state",
  component: RumahlEmptyState,
  args: {
    eyebrow: "Workspaces",
    title: "Room for something new",
    children: "Keep your apps and files together in a workspace of their own."
  },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The zero-content state of a page or list.

## Rules
- Explain what goes here and offer the **primary action** to add it.
- Keep it short and human; one clear CTA.`
      }
    }
  }
} satisfies Meta<typeof RumahlEmptyState>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithAction: Story = {
  args: { action: <Button variant="primary">Create workspace</Button> }
};
