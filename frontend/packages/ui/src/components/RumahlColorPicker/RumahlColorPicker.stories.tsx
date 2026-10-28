import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlColorPicker } from "@rumahl/ui/components";

const meta = {
  title: "Components/Color picker",
  component: RumahlColorPicker,
  args: { label: "Accent colour", size: "md" },
  argTypes: {
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    label: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `A labelled colour chooser with swatches.

## Rules
- Always labelled and showing the current value.
- Offer **curated swatches first**, custom second.`
      }
    }
  }
} satisfies Meta<typeof RumahlColorPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [value, setValue] = useState("#0B84FF");
    return <RumahlColorPicker {...args} value={value} onChange={setValue} />;
  }
};

export const WithNamedPresets: Story = {
  render: (args) => {
    const [value, setValue] = useState("#22C7A9");
    return (
      <RumahlColorPicker
        {...args}
        label="Highlight"
        value={value}
        onChange={setValue}
        presets={[
          { value: "#22C7A9", label: "Mint" },
          { value: "#F45B85", label: "Rose" },
          { value: "#0B84FF", label: "Blue" }
        ]}
      />
    );
  }
};

export const Sizes: Story = {
  render: (args) => (
    <div style={{ display: "grid", gap: 14, width: 280 }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlColorPicker {...args} key={size} size={size} label={`Size ${size}`} defaultValue="#0B84FF" />
      ))}
    </div>
  )
};
