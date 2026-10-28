import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Counter } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Counter",
  component: Counter,
  args: { value: 3, onChange: () => {}, min: 0, max: 10, step: 1, label: "Quantity", size: "md" },
  argTypes: {
    ...nativeArgTypes,
    value: { control: { type: "number" } },
    min: { control: { type: "number" } },
    max: { control: { type: "number" } },
    step: { control: { type: "number" } },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    disabled: { control: "boolean" },
    color: { control: "color" }
  },
  parameters: {
    docs: {
      description: {
        component: `A numeric stepper for small, bounded integer values.

## Rules
- For **small, bounded** numbers (quantity, count); free-form numbers use an Input.
- Always provide a \`label\`.
- Clamp or disable at the bounds rather than letting the value leave the range.`
      }
    }
  }
} satisfies Meta<typeof Counter>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [value, setValue] = useState(3);
    return <Counter {...args} value={value} onChange={setValue} />;
  }
};

export const Sizes: Story = {
  render: () => {
    const [value, setValue] = useState(3);
    return (
      <div style={{ display: "flex", gap: 14, alignItems: "center" }}>
        {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
          <Counter key={size} label="Quantity" value={value} onChange={setValue} min={0} max={10} size={size} />
        ))}
      </div>
    );
  }
};
