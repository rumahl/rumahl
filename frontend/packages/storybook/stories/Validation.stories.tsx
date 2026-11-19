import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlFieldGroup, RumahlInput, RumahlInputGroup, RumahlTextArea } from "@rumahl/ui/components";

const meta = { title: "Components/Validation", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const AllMaterials: Story = {
  render: (_args, context) => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 290px), 1fr))", gap: 24 }}>
      {(["glass", "floating", "solid"] as const).map(material => (
        <section className="shell" key={material} data-material={material} data-scheme={context.globals.scheme === "dark" ? "dark" : "light"}
          style={{ display: "grid", alignContent: "start", gap: 20, padding: 24, borderRadius: 24, color: "var(--rumahl-ui-color-text-primary)", background: "radial-gradient(ellipse at 100% 100%, #79b6ae20, transparent 70%), var(--rumahl-ui-color-panel-background)" }}>
          <h2 style={{ margin: 0, textTransform: "capitalize", fontSize: 20}}>{material}</h2>
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Neutral<RumahlInput defaultValue="Personal workspace" /></label>
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Error<RumahlInput defaultValue="hello@" error="Enter a complete email address." /></label>
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Success<RumahlInput defaultValue="hello@rumahl.local" status="success" /><small>✓ Address accepted</small></label>
          <RumahlInputGroup aria-label="Workspace address" prefix="/" defaultValue="workspace" status="success" />
          <RumahlInputGroup aria-label="Invalid workspace address" prefix="/" defaultValue="my workspace!" error="Use letters, numbers and hyphens." />
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Description<RumahlTextArea defaultValue="" error="Add a short description." /></label>
          <RumahlFieldGroup label="Verified settings" status="success" description="Both values have been checked.">
            <label>Name<RumahlInput defaultValue="My workspace" /></label>
            <label>Address<RumahlInput defaultValue="workspace.local" /></label>
          </RumahlFieldGroup>
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Disabled success<RumahlInput defaultValue="Verified" status="success" disabled /></label>
          <label style={{ display: "grid", gap: 8, fontSize: 12}}>Disabled error<RumahlInput defaultValue="Invalid value" status="error" disabled /></label>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}><Button variant="primary">Save changes</Button><Button variant="danger">Discard</Button></div>
        </section>
      ))}
    </div>
  )
};
