import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlStepper } from "@rumahl/ui/components";

const meta = {
  title: "Components/Stepper",
  component: RumahlStepper,
  args: {
    label: "Volume",
    decreaseLabel: "Decrease",
    increaseLabel: "Increase",
    value: 3,
    onChange: () => {},
    min: 0,
    max: 10
  },
  argTypes: { size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A numeric stepper with +/− controls (a labelled counter).

## Rules
- **Bounded values only**; label it and clamp at the bounds.
- For free-form numbers use an Input.`
      }
    }
  }
} satisfies Meta<typeof RumahlStepper>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [value, setValue] = useState(3);
    return <RumahlStepper {...args} value={value} onChange={setValue} />;
  }
};
