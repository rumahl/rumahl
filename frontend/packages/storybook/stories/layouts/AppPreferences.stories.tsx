import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  AppLayout,
  AppSidebar,
  GroupContainer,
  GroupRow,
  PageHeader,
  RumahlBadge,
  RumahlSwitch,
  RumahlTabs,
  SidenavItem,
  SidenavSection
} from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, PulseIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Preferences (tab-in-sidebar, app style)
 *
 * A template/inspiration: a sidebar for the top-level sections and tabs inside
 * the content for the sub-sections. Content on the surface.
 */
const meta = { title: "Layout/App/Preferences", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const NAV = [
  { id: "general", label: "General", icon: <SettingsIcon /> },
  { id: "appearance", label: "Appearance", icon: <DesktopIcon /> },
  { id: "accessibility", label: "Accessibility", icon: <PulseIcon /> },
  { id: "workspace", label: "Workspace", icon: <GridIcon /> }
];

export const Default: Story = {
  render: () => {
    const [section, setSection] = useState("appearance");
    const [tab, setTab] = useState("theme");
    return (
      <div style={{ height: 560 }}>
        <AppLayout
          sidebar={
            <AppSidebar>
              <SidenavSection title="Preferences">
                {NAV.map((item) => (
                  <SidenavItem
                    key={item.id}
                    as="button"
                    type="button"
                    icon={item.icon}
                    current={section === item.id}
                    onClick={() => setSection(item.id)}
                  >
                    {item.label}
                  </SidenavItem>
                ))}
              </SidenavSection>
            </AppSidebar>
          }
        >
          <PageHeader title="Appearance" description="Theme, wallpaper and accent colour.">
            <RumahlTabs
              label="Appearance sections"
              value={tab}
              onChange={setTab}
              tabs={[
                {
                  id: "theme",
                  label: "Theme",
                  content: (
                    <GroupContainer>
                      <GroupRow title="Dark mode">
                        <RumahlSwitch label="Dark mode" defaultChecked />
                      </GroupRow>
                      <GroupRow title="Accent" value={<RumahlBadge tone="success">Mint</RumahlBadge>} />
                    </GroupContainer>
                  )
                },
                {
                  id: "wallpaper",
                  label: "Wallpaper",
                  content: (
                    <GroupContainer>
                      <GroupRow title="Style" value="Default" />
                      <GroupRow title="Motion">
                        <RumahlSwitch label="Motion" defaultChecked />
                      </GroupRow>
                    </GroupContainer>
                  )
                }
              ]}
            />
          </PageHeader>
        </AppLayout>
      </div>
    );
  }
};
