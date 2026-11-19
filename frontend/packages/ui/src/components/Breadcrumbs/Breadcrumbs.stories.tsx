import type { Meta, StoryObj } from "@storybook/react-vite";
import { Breadcrumbs } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Breadcrumbs",
  component: Breadcrumbs,
  argTypes: nativeArgTypes,
  args: {
    items: [
      { label: "Home", href: "#" },
      { label: "Settings", href: "#" },
      { label: "Network" }
    ]
  },
  parameters: {
    docs: {
      description: {
        component: `The path to the current page within a hierarchy.

## Rules
- Reflect the **hierarchy**, not the browsing history.
- A few levels only; collapse the middle when it gets long.
- The last item is the current page — text, never a link.`
      }
    }
  }
} satisfies Meta<typeof Breadcrumbs>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
