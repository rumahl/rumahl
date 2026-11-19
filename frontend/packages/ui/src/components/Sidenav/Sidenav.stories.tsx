import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Sidenav, SidenavIdentity, SidenavItem, SidenavSection } from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, PulseIcon, SettingsIcon } from "@rumahl/ui/icons";

const meta = {
  title: "Components/Sidenav",
  component: Sidenav,
  args: { children: null },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The vertical app navigation (sidebar) with sections and items.

## Rules
- One level of sections; put the **primary** destinations here.
- The active item uses the accent; identity / header / footer are slots.
- Use \`AppSidebar\` inside \`AppLayout\`; for app shells with a navbar and mobile island use \`AppNavLayout\`.
- Keep labels short.`
      }
    }
  }
} satisfies Meta<typeof Sidenav>;
export default meta;
type Story = StoryObj<typeof meta>;

const NAV = [
  { id: "personalization", label: "Personalization", icon: <DesktopIcon /> },
  { id: "workspace", label: "Workspace", icon: <GridIcon /> },
  { id: "accessibility", label: "Accessibility", icon: <PulseIcon /> },
  { id: "system", label: "System", icon: <SettingsIcon /> }
];

const DEFAULT_CODE = `import { useState } from "react";
import { Sidenav, SidenavIdentity, SidenavItem, SidenavSection } from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, PulseIcon, SettingsIcon } from "@rumahl/ui/icons";

const NAV = [
  { id: "personalization", label: "Personalization", icon: <DesktopIcon /> },
  { id: "workspace", label: "Workspace", icon: <GridIcon /> },
  { id: "accessibility", label: "Accessibility", icon: <PulseIcon /> },
  { id: "system", label: "System", icon: <SettingsIcon /> }
];

function Story() {
  const [page, setPage] = useState("personalization");

  return (
    <Sidenav
      header={<SidenavIdentity name="Alex Morgan" detail="Personal · Default" />}
      footer={<>rumahl OS <b>Preview</b></>}
    >
      <SidenavSection title="User">
        {NAV.map((item) => (
          <SidenavItem
            key={item.id}
            as="button"
            type="button"
            icon={item.icon}
            current={page === item.id}
            onClick={() => setPage(item.id)}
          >
            {item.label}
          </SidenavItem>
        ))}
      </SidenavSection>
    </Sidenav>
  );
}`;

export const Default: Story = {
  parameters: { docs: { source: { language: "tsx", code: DEFAULT_CODE } } },
  render: () => {
    const [page, setPage] = useState("personalization");
    return (
      <div style={{ height: 420, width: 240 }}>
        <Sidenav
          header={<SidenavIdentity name="Alex Morgan" detail="Personal · Default" />}
          footer={<>rumahl OS <b>Preview</b></>}
        >
          <SidenavSection title="User">
            {NAV.map((item) => (
              <SidenavItem
                key={item.id}
                as="button"
                type="button"
                icon={item.icon}
                current={page === item.id}
                onClick={() => setPage(item.id)}
              >
                {item.label}
              </SidenavItem>
            ))}
          </SidenavSection>
        </Sidenav>
      </div>
    );
  }
};
