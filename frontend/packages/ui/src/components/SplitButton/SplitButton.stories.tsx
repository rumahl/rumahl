import type { Meta, StoryObj } from "@storybook/react-vite";
import { SplitButton } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/SplitButton",
  component: SplitButton,
  args: {
    children: "Deploy",
    variant: "primary",
    size: "md",
    actions: [
      { label: "Deploy to staging", onSelect: () => {} },
      { label: "Deploy and restart", onSelect: () => {} },
      { label: "Schedule deploy", onSelect: () => {} }
    ]
  },
  argTypes: {
    ...nativeArgTypes,
    variant: { control: "inline-radio", options: ["primary", "secondary", "ghost", "danger"] },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  },
  parameters: {
    docs: {
      description: {
        component: `A primary action plus a menu of related variants.

## Rules
- The main button is the **most likely** action; the menu holds the rest.
- Keep the menu to a few related options.
- Don't change the meaning of the main action between renders.`
      }
    }
  }
} satisfies Meta<typeof SplitButton>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
