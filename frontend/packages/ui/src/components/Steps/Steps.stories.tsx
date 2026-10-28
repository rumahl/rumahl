import type { Meta, StoryObj } from "@storybook/react-vite";
import { Steps } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const STEPS = [
  { id: "account", label: "Account" },
  { id: "device", label: "Device" },
  { id: "done", label: "Done" }
];

const meta = {
  title: "Components/Steps",
  component: Steps,
  args: { steps: STEPS, active: "device", orientation: "horizontal" },
  argTypes: { ...nativeArgTypes, orientation: { control: "inline-radio", options: ["horizontal", "vertical"] },
    color: { control: "color" } },
  parameters: {
    layout: "padded",
    docs: {
      description: {
        component: `A multi-step progress indicator (done / current / upcoming).

## Rules
- Show where the user is in a **sequence**; use Tabs for parallel views.
- Keep 3–5 steps with brief labels.
- Vertical on narrow / mobile, horizontal when wide.`
      }
    }
  }
} satisfies Meta<typeof Steps>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Horizontal: Story = {};
export const Vertical: Story = { args: { orientation: "vertical" } };
