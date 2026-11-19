import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlProgress, RumahlSpinner } from "@rumahl/ui/components";

const meta = {
  title: "Components/Progress",
  component: RumahlProgress,
  args: { label: "Storage", value: 42, size: "md" },
  argTypes: {
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    color: { control: "color" }
  },
  parameters: {
    docs: {
      description: {
        component: `A determinate progress bar (or indeterminate for unknown duration).

## Rules
- Show progress for **known-duration** work; a Spinner for unknown waits.
- Always label it and expose the value; never rely on colour alone.`
      }
    }
  }
} satisfies Meta<typeof RumahlProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Determinate: Story = {};
export const Full: Story = { args: { value: 100 } };
export const Overflow: Story = { args: { value: 150 } };
export const Indeterminate: Story = { render: () => <RumahlProgress label="Loading" /> };
export const Spinner: Story = { render: () => <RumahlSpinner label="Loading" /> };
export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16, width: 320 }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlProgress key={size} size={size} label={`Size ${size}`} value={64} />
      ))}
    </div>
  )
};
