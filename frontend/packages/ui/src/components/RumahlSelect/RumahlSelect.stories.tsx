import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlSelect } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const OPTIONS = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "auto", label: "Automatic" },
  { value: "high", label: "High contrast", warning: "May reduce battery life" }
];

const SELECT_DOCS = `Chooses one option from a list. Opens an anchored glass menu (see **Menu**),
styled like the OS.

- \`status\`, \`disabled\`, \`size\`, \`color\` / \`backgroundColor\`.
- The selected item is accent-tinted.

## Rules
- Use a **Select for 4+ options**; use a ButtonGroup / segmented control for 2–4.
- **Always provide a \`label\`** and keep options short.
- **Don't hide essential actions** in a Select — it is for choosing a value.`;

const meta = {
  title: "Components/Select",
  component: RumahlSelect,
  args: { label: "Appearance", options: OPTIONS, value: "light", onChange: () => {}, size: "md" },
  parameters: { docs: { description: { component: SELECT_DOCS } } },
  argTypes: {
    ...nativeArgTypes,
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    disabled: { control: "boolean" },
    status: { control: "inline-radio", options: ["neutral", "error", "success"] },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  }
} satisfies Meta<typeof RumahlSelect>;

export default meta;
type Story = StoryObj<typeof meta>;

const DEFAULT_CODE = `import { useState } from "react";
import { RumahlSelect } from "@rumahl/ui/components";

const OPTIONS = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "auto", label: "Automatic" }
];

function Story() {
  const [value, setValue] = useState("light");

  return (
    <RumahlSelect
      label="Appearance"
      options={OPTIONS}
      value={value}
      onChange={setValue}
    />
  );
}`;

export const Default: Story = {
  parameters: { docs: { source: { language: "tsx", code: DEFAULT_CODE } } },
  render: () => {
    const [value, setValue] = useState("light");
    return <RumahlSelect label="Appearance" options={OPTIONS} value={value} onChange={setValue} />;
  }
};

export const WithError: Story = {
  render: () => {
    const [value, setValue] = useState("light");
    return (
      <RumahlSelect
        label="Appearance"
        options={OPTIONS}
        value={value}
        onChange={setValue}
        error="Choose a valid option"
      />
    );
  }
};

export const Disabled: Story = {
  render: () => <RumahlSelect label="Appearance" options={OPTIONS} value="dark" onChange={() => {}} disabled />
};

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 14, maxWidth: 320 }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlSelect
          key={size}
          label={`Size ${size}`}
          options={OPTIONS}
          value="light"
          onChange={() => {}}
          size={size}
        />
      ))}
    </div>
  )
};

const MANY = Array.from({ length: 18 }, (_, index) => ({ value: `option-${index}`, label: `Option ${index + 1}` }));

export const LongList: Story = {
  render: () => {
    const [value, setValue] = useState("option-0");
    return (
      <div style={{ width: 320 }}>
        <RumahlSelect label="Many options" options={MANY} value={value} onChange={setValue} />
      </div>
    );
  }
};
