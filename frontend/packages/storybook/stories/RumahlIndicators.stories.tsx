import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlProgress, RumahlSpinner, RumahlRange, RumahlRadio, RumahlCheckbox, RumahlButtonGroup } from "@rumahl/ui/components";

const meta = { title: "Components/Indicators", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

function Indicators({ id }: { id: string }) {
  const [percent, setPercent] = useState(42);
  const [selection, setSelection] = useState("list");
  return <div style={{ display: "grid", gap: 24 }}>
    <RumahlRange label="Progress" min={0} max={100} value={percent} onChange={event => setPercent(event.target.valueAsNumber)} />
    <RumahlProgress label="Storage used" value={percent} />
    <RumahlProgress label="Empty" value={0} />
    <RumahlProgress label="Complete" value={100} />
    <RumahlProgress label="Loading files" />
    <div style={{ display: "flex", alignItems: "center", gap: 18 }}>
      <RumahlSpinner label="Loading small" size="sm" /><RumahlSpinner label="Loading" /><RumahlSpinner label="Loading large" size="lg" />
    </div>
    <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
      <RumahlCheckbox label="Selected" defaultChecked /><RumahlCheckbox label="Empty" /><RumahlCheckbox label="Disabled" defaultChecked disabled />
      <RumahlRadio name={id} label="First" defaultChecked /><RumahlRadio name={id} label="Second" />
    </div>
    <RumahlButtonGroup label="Drag to choose a view" value={selection} onChange={setSelection} options={[
      { value: "list", label: "List" }, { value: "grid", label: "Grid" },
      { value: "locked", label: "Locked", disabled: true }, { value: "columns", label: "Columns" }
    ]} />
  </div>;
}
export const AllMaterials: Story = {
  render: (_args, context) => <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 300px), 1fr))", gap: 24 }}>
    {(["glass", "floating", "solid"] as const).map(material => <section key={material} className="shell" data-material={material} data-scheme={context.globals.scheme === "dark" ? "dark" : "light"}
      style={{ padding: 24, borderRadius: 24, background: "var(--rumahl-ui-color-panel-background)", color: "var(--rumahl-ui-color-text-primary)" }}>
      <h2 style={{ margin: "0 0 24px", fontSize: 20, textTransform: "capitalize" }}>{material}</h2>
      <Indicators id={`indicator-${material}`} />
    </section>)}
  </div>
};
export const Spinner: Story = { render: () => <RumahlSpinner label="Loading workspace" size="lg" /> };
