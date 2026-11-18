import type { Meta, StoryObj } from "@storybook/react-vite";
import { Heading, IconButton, List, ListItem, PhoneFrame, RumahlAvatar, RumahlBadge } from "@rumahl/ui/components";
import { BoltIcon, HomeIcon, SearchIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Mobile shell (hybrid)
 *
 * A template/inspiration for a phone surface: a status bar, content and a
 * bottom tab bar inside a `PhoneFrame`. rumahl OS opens on a phone inside the
 * launcher; a companion mobile app can reuse the same components.
 */
const meta = { title: "Layout/Mobile/Shell", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const TABS = [
  { id: "home", icon: <HomeIcon />, label: "Home" },
  { id: "search", icon: <SearchIcon />, label: "Search" },
  { id: "activity", icon: <BoltIcon />, label: "Activity" },
  { id: "settings", icon: <SettingsIcon />, label: "Settings" }
];

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
        footer={
          <div style={{ display: "flex", justifyContent: "space-around", padding: 8 }}>
            {TABS.map((tab, index) => (
              <IconButton key={tab.id} shape="circle" variant={index === 0 ? "primary" : "ghost"} label={tab.label}>
                {tab.icon}
              </IconButton>
            ))}
          </div>
        }
      >
        <div style={{ padding: "8px 18px" }}>
          <Heading level={3} style={{ marginBottom: 12 }}>
            Good morning
          </Heading>
          <List>
            <ListItem leading={<RumahlAvatar name="Alex Morgan" size="sm" />} title="Alex Morgan" subtitle="Signed in" />
            <ListItem title="Focus" subtitle="On until 12:00" trailing={<RumahlBadge tone="success">On</RumahlBadge>} />
            <ListItem title="Backups" subtitle="Last run 2 hours ago" trailing={<RumahlBadge>OK</RumahlBadge>} />
            <ListItem title="Storage" subtitle="128 GB of 512 GB used" />
          </List>
        </div>
      </PhoneFrame>
    </div>
  )
};
