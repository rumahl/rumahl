import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, Divider, Flex, PageHeader, RumahlInput, RumahlSelect, RumahlSwitch, Steps, Text } from "@rumahl/ui/components";

/**
 * # Onboarding / wizard (app style)
 *
 * A template/inspiration for a guided setup: a step indicator, one form per
 * step and a footer with Back/Continue. Content on the surface.
 */
const meta = { title: "Layout/App/Onboarding", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const STEPS = [
  { id: "device", label: "Device" },
  { id: "account", label: "Account" },
  { id: "done", label: "Done" }
];

export const Default: Story = {
  render: () => {
    const [step, setStep] = useState("device");
    const index = STEPS.findIndex((s) => s.id === step);
    const next = () => setStep(STEPS[Math.min(index + 1, STEPS.length - 1)]!.id);
    const back = () => setStep(STEPS[Math.max(index - 1, 0)]!.id);
    return (
      <div style={{ height: 560, display: "flex", flexDirection: "column" }}>
        <div style={{ padding: "22px 30px 0" }}>
          <Steps steps={STEPS} active={step} />
        </div>
        <div style={{ flex: 1, overflow: "auto", padding: "24px 30px", maxWidth: 560, display: "grid", gap: 16, alignContent: "start" }}>
          {step === "device" ? (
            <>
              <PageHeader title="Set up this device" description="Name it and choose where it lives." />
              <label style={{ display: "grid", gap: 8 }}>
                <Text size="sm">Device name</Text>
                <RumahlInput defaultValue="Alex's phone" />
              </label>
              <label style={{ display: "grid", gap: 8 }}>
                <Text size="sm">Region</Text>
                <RumahlSelect
                  label="Region"
                  value="eu"
                  onChange={() => {}}
                  options={[
                    { value: "eu", label: "Europe" },
                    { value: "us", label: "United States" }
                  ]}
                />
              </label>
            </>
          ) : step === "account" ? (
            <>
              <PageHeader title="Your account" description="Sign in to sync your workspace." />
              <label style={{ display: "grid", gap: 8 }}>
                <Text size="sm">Email</Text>
                <RumahlInput type="email" defaultValue="alex@rumahl.dev" />
              </label>
              <RumahlSwitch label="Send me product updates" defaultChecked />
            </>
          ) : (
            <PageHeader title="You're all set" description="Your device is ready to use." />
          )}
        </div>
        <Divider />
        <Flex align="center" justify="between" gap={3} style={{ padding: 16 }}>
          <Button onClick={back} disabled={index === 0}>
            Back
          </Button>
          <Button variant="primary" onClick={next} disabled={index === STEPS.length - 1}>
            Continue
          </Button>
        </Flex>
      </div>
    );
  }
};
