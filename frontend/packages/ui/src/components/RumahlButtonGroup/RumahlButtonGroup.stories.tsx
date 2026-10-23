import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlButtonGroup } from "@rumahl/ui/components";

const OPTIONS = [
  { value: "list", label: "List" },
  { value: "grid", label: "Grid" },
  { value: "columns", label: "Columns", disabled: true }
];

const meta = {
  title: "Components/Button group",
  parameters: {
    docs: {
      description: {
        component: `Segmented actions that read as one control.

## Rules
- 2–5 mutually exclusive choices (or a small toolbar).
- The selected option is accent-tinted; keep labels parallel.
- For 4+ options, consider a **Select**.`
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const SingleSelection: Story = {
  render: () => {
    const [value, setValue] = useState("list");
    return <RumahlButtonGroup label="Layout" options={OPTIONS} value={value} onChange={setValue} />;
  }
};

export const MultipleToggle: Story = {
  render: () => {
    const [value, setValue] = useState<readonly string[]>(["grid"]);
    return (
      <RumahlButtonGroup
        label="Views"
        multiple
        options={OPTIONS}
        value={value}
        onChange={(next) => setValue(next)}
      />
    );
  }
};

export const Actions: Story = {
  render: () => (
    <RumahlButtonGroup
      label="Actions"
      mode="actions"
      options={[
        { value: "cut", label: "Cut" },
        { value: "copy", label: "Copy" },
        { value: "paste", label: "Paste" }
      ]}
      onChange={() => {}}
    />
  )
};

function SelectionComparison() {
  const [single, setSingle] = useState("list");
  const [multiple, setMultiple] = useState<string[]>(["list", "grid"]);
  return <div style={{ display: "grid", gap: 24 }}>
    <section><h3>Single selection · drag to move</h3>
      <RumahlButtonGroup label="Single view" options={OPTIONS} value={single} onChange={setSingle} />
    </section>
    <section><h3>Multiple selection · toggle independently</h3>
      <RumahlButtonGroup label="Visible views" options={OPTIONS} multiple value={multiple} onChange={setMultiple} />
    </section>
  </div>;
}
export const SingleAndMultiple: Story = { render: () => <SelectionComparison /> };

const SIZES = ["xs", "sm", "md", "lg", "xl"] as const;
export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16 }}>
      {SIZES.map((size) => (
        <RumahlButtonGroup
          key={size}
          label={`Layout (${size})`}
          size={size}
          options={OPTIONS}
          value="list"
          onChange={() => {}}
        />
      ))}
    </div>
  )
};
