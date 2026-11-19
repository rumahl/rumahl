import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlPageHeading } from "@rumahl/ui/components";
import { ArrowLeftIcon } from "@rumahl/ui/icons";

const meta = {
  title: "Components/Page heading",
  component: RumahlPageHeading,
  args: { title: "Appearance", help: "Choose how your workspace looks and feels." },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The title block of a page or section.

## Rules
- **One H1 per page**, with an optional help / description line.
- Page actions sit to the right of the heading.`
      }
    }
  }
} satisfies Meta<typeof RumahlPageHeading>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithBack: Story = {
  args: { back: <Button aria-label="Back"><ArrowLeftIcon /></Button> }
};
export const Long: Story = {
  args: {
    title: "Workspace appearance and accessibility",
    help: "Long headings and descriptions wrap naturally on narrow windows."
  }
};
