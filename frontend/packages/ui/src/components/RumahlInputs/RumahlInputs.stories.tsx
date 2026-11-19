import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  RumahlCheckbox,
  RumahlFieldGroup,
  RumahlFileInput,
  RumahlInput,
  RumahlInputGroup,
  RumahlMultiSelect,
  RumahlRange,
  RumahlRadio,
  RumahlSwitch,
  RumahlTextArea
} from "@rumahl/ui/components";

const INPUTS_DOCS = `Text entry. Wraps a native \`<input>\`; the label and hint are slots.

- \`glass\` — recessed on a surface (default); \`glass={false}\` — flat.
- \`floating\` — over a picture or another surface (its own island).
- \`status\` (neutral / error / success), \`loading\`, \`disabled\`, \`size\` xs–xl.

## Rules
- **Always pair with a \`label\`** (or an accessible name) — placeholders are not labels.
- **Errors are text, not colour.** Use \`status="error"\` with a hint, never colour alone.
- Native props (\`value\`, \`onChange\`, \`name\`, \`type\`…) are forwarded.`;

const meta = {
  title: "Components/Inputs",
  component: RumahlInput,
  args: { placeholder: "Type here", size: "md", glass: true, disabled: false },
  parameters: { docs: { description: { component: INPUTS_DOCS } } },
  argTypes: {
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    glass: { control: "boolean" },
    floating: { control: "boolean" },
    status: { control: "inline-radio", options: ["neutral", "error", "success"] },
    disabled: { control: "boolean" },
    loading: { control: "boolean" },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  }
} satisfies Meta<typeof RumahlInput>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Text: Story = { args: {} };
export const Sizes: Story = {
  render: (args) => (
    <div style={{ display: "grid", gap: 12 }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlInput {...args} key={size} size={size} placeholder={`Size ${size}`} />
      ))}
    </div>
  )
};
export const TextValidated: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12 }}>
      <RumahlInput placeholder="Success" status="success" defaultValue="ok" />
      <RumahlInput placeholder="Error" error="This value is required" defaultValue="" />
    </div>
  )
};
export const Glass: Story = { render: () => <RumahlInput material="glass" defaultValue="Glass input" /> };
export const Disabled: Story = { render: () => <RumahlInput disabled defaultValue="Disabled" /> };

export const TextArea: Story = {
  render: () => <RumahlTextArea rows={3} placeholder="Multiline" defaultValue={"Line one\nLine two"} />
};

export const Checkbox: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 8 }}>
      <RumahlCheckbox label="Unchecked" />
      <RumahlCheckbox label="Checked" defaultChecked />
      <RumahlCheckbox label="Disabled" disabled />
    </div>
  )
};

export const Radio: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 8 }}>
      <RumahlRadio name="demo" label="Option A" defaultChecked />
      <RumahlRadio name="demo" label="Option B" />
    </div>
  )
};

export const InputGroup: Story = {
  render: () => (
    <RumahlInputGroup prefix="$" suffix="/mo" defaultValue="12" aria-label="Amount" />
  )
};

export const FieldGroup: Story = {
  render: () => (
    <RumahlFieldGroup label="Network" description="Applied on the next connection.">
      <RumahlSwitch label="Automatic" defaultChecked />
      <RumahlSwitch label="Metered" />
    </RumahlFieldGroup>
  )
};

export const MultiSelect: Story = {
  render: () => {
    const [value, setValue] = useState<readonly string[]>(["wifi"]);
    return (
      <RumahlMultiSelect
        label="Interfaces"
        description="Pick any that apply."
        value={value}
        onChange={setValue}
        options={[
          { value: "wifi", label: "Wi-Fi" },
          { value: "ethernet", label: "Ethernet" },
          { value: "vpn", label: "VPN", disabled: true }
        ]}
      />
    );
  }
};

export const Switch: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 10 }}>
      <RumahlSwitch label="On" defaultChecked />
      <RumahlSwitch label="Off" />
      <RumahlSwitch label="Disabled" disabled />
    </div>
  )
};

export const Range: Story = {
  render: () => <RumahlRange label="Opacity" min={0} max={100} defaultValue={60} />
};

export const File: Story = {
  render: () => <RumahlFileInput label="Choose file" />
};

export const Tinted: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12, width: 320 }}>
      {["#0b84ff", "#22c7a9", "#f45b85", "#ff9f0a"].map((color) => (
        <RumahlInput key={color} color={color} placeholder={color} defaultValue={color} />
      ))}
    </div>
  )
};

export const Loading: Story = {
  render: () => (
    <div style={{ width: 320 }}>
      <RumahlInput loading defaultValue="Fetching value…" placeholder="Loading" />
    </div>
  )
};

export const Required: Story = {
  render: () => {
    const [value, setValue] = useState("");
    const [touched, setTouched] = useState(false);
    const error = touched && !value.trim() ? "This field is required" : undefined;
    return (
      <RumahlInput
        placeholder="Required field"
        value={value}
        onBlur={() => setTouched(true)}
        {...(error ? { error } : {})}
        onChange={(event) => setValue(event.target.value)}
      />
    );
  }
};

export const States: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 14, width: 340 }}>
      <RumahlInput placeholder="Default" />
      <RumahlInput placeholder="Success" status="success" defaultValue="ok" />
      <RumahlInput placeholder="Error" error="This field is required" defaultValue="x" />
      <RumahlInput placeholder="Disabled" disabled defaultValue="Cannot edit" />
      <RumahlSwitch label="Switch on" defaultChecked />
      <RumahlSwitch label="Switch off" />
      <RumahlCheckbox label="Checkbox checked" defaultChecked />
      <RumahlRadio label="Radio selected" defaultChecked />
      <RumahlRange label="Range" defaultValue={40} />
    </div>
  )
};
