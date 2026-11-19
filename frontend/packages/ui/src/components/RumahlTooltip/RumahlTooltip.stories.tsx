import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlTooltip } from "@rumahl/ui/components";

const meta = {
  title: "Components/Tooltip",
  component: RumahlTooltip,
  args: { content: "Tooltip content", children: <Button>Hover or focus me</Button> },
  argTypes: {
    content: { control: "text" },
    children: { control: false }
  },
  parameters: {
    docs: {
      description: {
        component: `A short, non-interactive hint shown on hover / focus.

## Rules
- For **labels and clarifications**, never essential or interactive content.
- Keep it to one line.
- Never the only place information exists — also expose it accessibly.`
      }
    }
  }
} satisfies Meta<typeof RumahlTooltip>;
export default meta;
type Story = StoryObj<typeof meta>;

export const OnButton: Story = {};
