import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlSearch } from "@rumahl/ui/components";

const meta = {
  title: "Components/Search",
  component: RumahlSearch,
  args: { label: "Search apps", clearLabel: "Clear search", value: "", onChange: () => {} },
  argTypes: { size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A labelled search field with a clear affordance.

## Rules
- It is an input — **always label it** (a placeholder is not a label).
- Provide a clear action; debounce live filtering.`
      }
    }
  }
} satisfies Meta<typeof RumahlSearch>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [value, setValue] = useState("rumahl");
    return <RumahlSearch {...args} value={value} onChange={setValue} />;
  }
};

export const Sizes: Story = {
  render: () => {
    const [value, setValue] = useState("rumahl");
    return (
      <div style={{ display: "grid", gap: 14, width: 320 }}>
        {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
          <RumahlSearch
            key={size}
            label={`Search (${size})`}
            clearLabel="Clear search"
            value={value}
            onChange={setValue}
            size={size}
          />
        ))}
      </div>
    );
  }
};
