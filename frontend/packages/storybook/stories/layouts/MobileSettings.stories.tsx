import type { Meta, StoryObj } from "@storybook/react-vite";
import { GroupContainer, GroupRow, Heading, IconButton, PhoneFrame, RumahlSwitch, Text } from "@rumahl/ui/components";
import { ArrowLeftIcon } from "@rumahl/ui/icons";

/**
 * # Mobile settings (hybrid)
 *
 * A template/inspiration for a phone settings screen: a nav title and grouped
 * rows inside a `PhoneFrame`. Same components as the desktop, arranged for a
 * small viewport.
 */
const meta = { title: "Layout/Mobile/Settings", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div style={{ minHeight: 700, display: "grid", placeItems: "center", padding: 24, background: "var(--rumahl-ui-color-canvas-background)" }}>
      <PhoneFrame
        status={
          <>
            <span>9:41</span>
            <span style={{ letterSpacing: 1, opacity: 0.7 }}>••• ▮</span>
          </>
        }
      >
        <div style={{ padding: "0 18px 24px", display: "grid", gap: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 0" }}>
            <IconButton shape="circle" variant="ghost" label="Back">
              <ArrowLeftIcon />
            </IconButton>
            <Text weight="semibold">Settings</Text>
            <span style={{ width: 30 }} />
          </div>

          <Heading level={6} tone="muted">Appearance</Heading>
          <GroupContainer>
            <GroupRow title="Dark mode">
              <RumahlSwitch label="Dark mode" defaultChecked />
            </GroupRow>
            <GroupRow title="Theme" value="Default" />
          </GroupContainer>

          <Heading level={6} tone="muted" style={{ marginTop: 2 }}>Device</Heading>
          <GroupContainer>
            <GroupRow title="Name" value="Alex's phone" />
            <GroupRow title="Storage" value="128 / 512 GB" />
            <GroupRow title="Notifications">
              <RumahlSwitch label="Notifications" />
            </GroupRow>
          </GroupContainer>
        </div>
      </PhoneFrame>
    </div>
  )
};
