import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlTabs } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Tabs",
  component: RumahlTabs,
  args: { label: "Settings sections", value: "general", onChange: () => {}, tabs: [] },
  argTypes: { ...nativeArgTypes, size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `Switch between related views in place.

## Rules
- Tabs switch **content in place** — they are not navigation.
- 2–7 short, parallel labels.
- The active tab is clear and its shared pill animates in place.
- For a sequence use **Steps**, not tabs.`
      }
    }
  }
} satisfies Meta<typeof RumahlTabs>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [value, setValue] = useState("general");
    return (
      <div style={{ width: 420 }}>
        <RumahlTabs
          {...args}
          value={value}
          onChange={setValue}
          tabs={[
            { id: "general", label: "General", content: <p>General settings.</p> },
            { id: "network", label: "Network", content: <p>Network settings.</p> },
            { id: "about", label: "About", content: <p>About this device.</p>, disabled: true }
          ]}
        />
      </div>
    );
  }
};

export const Sizes: Story = {
  render: () => {
    const [value, setValue] = useState("a");
    return (
      <div style={{ display: "grid", gap: 16, width: 440 }}>
        {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
          <RumahlTabs
            key={size}
            size={size}
            label={`Sections ${size}`}
            value={value}
            onChange={setValue}
            tabs={[
              { id: "a", label: "General", content: <p>General.</p> },
              { id: "b", label: "Network", content: <p>Network.</p> }
            ]}
          />
        ))}
      </div>
    );
  }
};

export const Many: Story = {
  render: () => {
    const [value, setValue] = useState("overview");
    return (
      <div style={{ width: 620 }}>
        <RumahlTabs
          label="Sections"
          value={value}
          onChange={setValue}
          tabs={[
            { id: "overview", label: "Overview", content: <p>Overview.</p> },
            { id: "activity", label: "Activity", content: <p>Activity.</p> },
            { id: "members", label: "Members", content: <p>Members.</p> },
            { id: "billing", label: "Billing", content: <p>Billing.</p> },
            { id: "settings", label: "Settings", content: <p>Settings.</p> },
            { id: "archive", label: "Archive", content: <p>Archive.</p>, disabled: true }
          ]}
        />
      </div>
    );
  }
};
