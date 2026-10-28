import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlCalendar, RumahlDatePicker } from "@rumahl/ui/components";
import type { ControlSize } from "@rumahl/ui";

const SIZE_OPTIONS = ["xs", "sm", "md", "lg", "xl"] as const;

const meta = {
  title: "Components/Calendar",
  args: { size: "md" as ControlSize },
  argTypes: {
    size: { control: "inline-radio", options: SIZE_OPTIONS }
  },
  parameters: {
    docs: {
      description: {
        component: `A date picker calendar.

## Rules
- One month at a time; **today** and the **selected** day are visually distinct.
- Pair with a labelled field; never signal the selection by colour alone.`
      }
    }
  }
} satisfies Meta<{ size?: ControlSize }>;
export default meta;
type Story = StoryObj<{ size?: ControlSize }>;

export const Calendar: Story = {
  render: (args) => {
    const [value, setValue] = useState("2026-10-14");
    return <RumahlCalendar value={value} onChange={setValue} size={args.size} />;
  }
};

export const DatePicker: Story = {
  render: (args) => {
    const [value, setValue] = useState("2026-10-14");
    return <RumahlDatePicker label="Scheduled date" value={value} onChange={setValue} size={args.size} />;
  }
};

export const Bounded: Story = {
  render: (args) => {
    const [value, setValue] = useState("2026-10-14");
    return (
      <RumahlDatePicker
        label="Within a range"
        min="2026-10-01"
        max="2026-10-31"
        value={value}
        onChange={setValue}
        size={args.size}
      />
    );
  }
};

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16 }}>
      {SIZE_OPTIONS.map((size) => (
        <RumahlDatePicker
          key={size}
          label={`Size ${size}`}
          value="2026-10-14"
          onChange={() => {}}
          size={size}
        />
      ))}
    </div>
  )
};
