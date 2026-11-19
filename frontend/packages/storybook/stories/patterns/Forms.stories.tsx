import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Button,
  RumahlFieldGroup,
  RumahlInput,
  RumahlSelect,
  RumahlSwitch
} from "@rumahl/ui/components";

const FORMS_DOCS = `# Forms

Compose a settings form from fields. Each field owns its label and its
validation feedback; the form only wires state and submit. Validation is shown
with \`error\` (border + ring + message), never by tinting the inset.

## Do
- **Label every field** (\`RumahlFieldGroup\` + the field's \`label\`) — a
  placeholder is not a label.
- **Validate on submit or blur**, not on every keystroke, and put the message
  next to the field via \`error\`.
- **One primary action** (Save); Reset / Cancel stay secondary.
- **Group related fields** with \`RumahlFieldGroup\`; keep the form short.

## Don't
- Don't signal an error with colour alone — pair it with text.
- Don't disable the submit button to "prevent" errors; show them instead.
- Don't use a placeholder as the only label.`;

const meta = {
  title: "Patterns/Forms",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: { description: { component: FORMS_DOCS } }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function SettingsForm() {
  const [name, setName] = useState("");
  const [region, setRegion] = useState("eu");
  const [telemetry, setTelemetry] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const nameError = submitted && !name.trim() ? "Please enter a name" : undefined;
  return (
    <form
      style={{ display: "grid", gap: 18, width: 420 }}
      onSubmit={(event) => {
        event.preventDefault();
        setSubmitted(true);
      }}
    >
      <RumahlFieldGroup label="Device" description="How this device is identified.">
        <RumahlInput
          placeholder="Device name"
          value={name}
          {...(nameError ? { error: nameError } : {})}
          onChange={(event) => setName(event.target.value)}
        />
        <RumahlSelect
          label="Region"
          value={region}
          onChange={setRegion}
          options={[
            { value: "eu", label: "Europe" },
            { value: "us", label: "United States" },
            { value: "apac", label: "Asia Pacific" }
          ]}
        />
        <RumahlSwitch
          label="Share anonymous telemetry"
          checked={telemetry}
          onChange={(event) => setTelemetry(event.target.checked)}
        />
      </RumahlFieldGroup>
      <div style={{ display: "flex", gap: 10 }}>
        <Button variant="primary" type="submit">Save</Button>
        <Button type="reset">Reset</Button>
      </div>
    </form>
  );
}

export const Settings: Story = {
  render: () => <SettingsForm />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { useState } from "react";
import {
  Button,
  RumahlFieldGroup,
  RumahlInput,
  RumahlSelect,
  RumahlSwitch
} from "@rumahl/ui/components";

export function DeviceSettings() {
  const [name, setName] = useState("");
  const [region, setRegion] = useState("eu");
  const [telemetry, setTelemetry] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const nameError = submitted && !name.trim() ? "Please enter a name" : undefined;
  return (
    <form
      style={{ display: "grid", gap: 18, width: 420 }}
      onSubmit={(event) => {
        event.preventDefault();
        setSubmitted(true);
      }}
    >
      <RumahlFieldGroup label="Device" description="How this device is identified.">
        <RumahlInput
          placeholder="Device name"
          value={name}
          error={nameError}
          onChange={(event) => setName(event.target.value)}
        />
        <RumahlSelect
          label="Region"
          value={region}
          onChange={setRegion}
          options={[
            { value: "eu", label: "Europe" },
            { value: "us", label: "United States" },
            { value: "apac", label: "Asia Pacific" }
          ]}
        />
        <RumahlSwitch
          label="Share anonymous telemetry"
          checked={telemetry}
          onChange={(event) => setTelemetry(event.target.checked)}
        />
      </RumahlFieldGroup>
      <div style={{ display: "flex", gap: 10 }}>
        <Button variant="primary" type="submit">Save</Button>
        <Button type="reset">Reset</Button>
      </div>
    </form>
  );
}`
      }
    }
  }
};
