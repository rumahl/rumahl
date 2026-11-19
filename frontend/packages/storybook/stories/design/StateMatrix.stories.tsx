import type { ReactNode } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, Chip, RumahlInput, RumahlSelect, RumahlSwitch } from "@rumahl/ui/components";

/**
 * # State matrix
 *
 * The shared state vocabulary rendered with the active theme. Use the theme
 * toolbar to inspect Iris, Ground and derived themes; the states themselves must
 * never change meaning.
 */
const meta = {
  title: "Design System/State matrix",
  parameters: { layout: "fullscreen" }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div style={{ display: "grid", gridTemplateColumns: "150px minmax(0, 1fr)", alignItems: "center", gap: 16 }}>
      <span style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{label}</span>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>{children}</div>
    </div>
  );
}

function States() {
  return (
    <div style={{ display: "grid", gap: 18, padding: 28, maxWidth: 900 }}>
      <Row label="Button variants">
        <Button variant="primary">Primary</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="danger">Danger</Button>
      </Row>
      <Row label="Button states">
        <Button variant="primary" loading>Working</Button>
        <Button variant="primary" active>Selected</Button>
        <Button variant="secondary" disabled>Disabled</Button>
      </Row>
      <Row label="Chips">
        <Chip>Default</Chip>
        <Chip selected>Selected</Chip>
        <Chip disabled>Disabled</Chip>
      </Row>
      <Row label="Inputs">
        <div style={{ width: 220 }}><RumahlInput aria-label="Default input" placeholder="Default" /></div>
        <div style={{ width: 220 }}><RumahlInput aria-label="Error input" status="error" defaultValue="Invalid value" /></div>
        <div style={{ width: 220 }}><RumahlInput aria-label="Disabled input" disabled defaultValue="Disabled" /></div>
      </Row>
      <Row label="Select">
        <div style={{ width: 220 }}>
          <RumahlSelect
            label="Example"
            value="one"
            options={[
              { value: "one", label: "One" },
              { value: "two", label: "Two" }
            ]}
            onChange={() => {}}
          />
        </div>
      </Row>
      <Row label="Switch">
        <RumahlSwitch label="Enabled switch" defaultChecked />
        <RumahlSwitch label="Disabled switch" disabled />
      </Row>
      <Row label="Feedback">
        <span className="rumahl-badge">Neutral</span>
        <span className="rumahl-badge" style={{ color: "var(--rumahl-ui-color-success)", background: "var(--rumahl-ui-color-success-surface)" }}>Success</span>
        <span className="rumahl-badge" style={{ color: "var(--rumahl-ui-color-warning)", background: "var(--rumahl-ui-color-warning-surface)" }}>Warning</span>
        <span className="rumahl-badge" style={{ color: "var(--rumahl-ui-color-danger)", background: "var(--rumahl-ui-color-danger-surface)" }}>Danger</span>
        <span className="rumahl-badge" style={{ color: "var(--rumahl-ui-color-info)", background: "var(--rumahl-ui-color-info-surface)" }}>Info</span>
      </Row>
    </div>
  );
}

export const AllStates: Story = {
  render: () => <States />
};
