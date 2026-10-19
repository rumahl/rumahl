import { useState, type ReactNode } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  AppLayout,
  AppSidebar,
  GroupContainer,
  GroupLink,
  GroupLinkContainer,
  GroupRow,
  PageHeader,
  PageSection,
  RumahlSearch,
  RumahlSwitch,
  SidenavItem,
  SidenavSection
} from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, PulseIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Sidebar-based application layout
 *
 * A design idea for composing a screen from the components — not a fixed
 * structure. `AppLayout` takes any sidebar (or none); the sidebar header and
 * footer are ordinary slots (a search, a dropdown, a profile … or nothing),
 * and everything inside the sidebar/content is up to the app.
 */
const meta = {
  title: "Layout/App/Sidebar layout",
  component: AppLayout,
  args: { children: null, collapsed: false },
  argTypes: {
    collapsed: { control: "boolean" },
    sidebar: { control: false },
    children: { control: false }
  },
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: `Two-pane app frame: a sidebar (\`AppSidebar\`) and a content scroller. The layout
brings the native window surface; the sidebar is the \`--side\` surface and the
content is transparent (the window glass shows through).

- \`collapsible\` / \`collapsed\` / \`defaultCollapsed\` — an icon-only rail.
- Sidebar header / footer are slots (a search, a profile… or nothing).

## Rules
- **One sidebar, one content area.** Put the top-level sections in the sidebar.
- Compose content from \`PageHeader\` + \`PageSection\` +
  \`GroupContainer\`/\`GroupRow\` (or \`GroupLinkContainer\`).
- **No material on the content** — material is for the sidebar and the frame.
- A template / inspiration, not a mandate.`
      }
    }
  }
} satisfies Meta<typeof AppLayout>;
export default meta;
type Story = StoryObj<typeof meta>;

const NAV = [
  { id: "personalization", label: "Personalization", icon: <DesktopIcon /> },
  { id: "workspace", label: "Workspace", icon: <GridIcon /> },
  { id: "accessibility", label: "Accessibility", icon: <PulseIcon /> },
  { id: "system", label: "System", icon: <SettingsIcon /> }
];

const DEFAULT_CODE = `import { useState } from "react";
import {
  AppLayout,
  AppSidebar,
  GroupContainer,
  GroupRow,
  PageHeader,
  PageSection,
  RumahlSwitch,
  SidenavItem,
  SidenavSection
} from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, SettingsIcon } from "@rumahl/ui/icons";

const NAV = [
  { id: "personalization", label: "Personalization", icon: <DesktopIcon /> },
  { id: "workspace", label: "Workspace", icon: <GridIcon /> },
  { id: "system", label: "System", icon: <SettingsIcon /> }
];

export function App() {
  const [page, setPage] = useState("personalization");

  return (
    <AppLayout
      collapsible
      sidebar={
        <AppSidebar>
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
        </AppSidebar>
      }
    >
      <PageHeader title="Personalization" description="Choose how rumahl OS looks and feels.">
        <PageSection title="Appearance">
          <GroupContainer>
            <GroupRow title="Dark mode">
              <RumahlSwitch label="Dark mode" defaultChecked />
            </GroupRow>
            <GroupRow title="Theme" value="Default" />
          </GroupContainer>
        </PageSection>
      </PageHeader>
    </AppLayout>
  );
}`;

function renderLayout(args: { collapsed?: boolean | undefined }, header?: ReactNode) {
  const [page, setPage] = useState("personalization");
  return (
    <div style={{ height: 560 }}>
      <AppLayout
        collapsible
        defaultCollapsed={args.collapsed}
        sidebar={
          <AppSidebar {...(header ? { header } : {})}>
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
          </AppSidebar>
        }
      >
        <PageHeader title="Personalization" description="Choose how rumahl OS looks and feels.">
          <PageSection title="Appearance">
            <GroupContainer>
              <GroupRow title="Dark mode">
                <RumahlSwitch label="Dark mode" defaultChecked />
              </GroupRow>
              <GroupRow title="Theme" value="Default" />
            </GroupContainer>
          </PageSection>
          <PageSection title="Links">
            <GroupLinkContainer>
              <GroupLink icon={<GridIcon />} title="Display" subtitle="Resolution and scaling" href="#display" />
              <GroupLink icon={<PulseIcon />} title="Accessibility" subtitle="Motion and contrast" href="#a11y" />
            </GroupLinkContainer>
          </PageSection>
        </PageHeader>
      </AppLayout>
    </div>
  );
}

export const Default: Story = {
  parameters: { docs: { source: { language: "tsx", code: DEFAULT_CODE } } },
  render: (args) => renderLayout(args)
};

/** The sidebar header is a slot — put anything here (or nothing). */
export const WithHeader: Story = {
  parameters: { docs: { source: { language: "tsx", code: DEFAULT_CODE } } },
  render: () => renderLayout({}, <RumahlSearch label="Search settings" clearLabel="Clear" value="" onChange={() => {}} />)
};

/** Collapsed rail — the layout flexes to an icon-only sidebar. */
export const Collapsed: Story = {
  args: { collapsed: true },
  parameters: { docs: { source: { language: "tsx", code: DEFAULT_CODE } } },
  render: (args) => renderLayout(args)
};

/** Same layout without a sidebar — just the content pane. */
export const ContentOnly: Story = {
  render: () => (
    <div style={{ height: 360 }}>
      <AppLayout>
        <PageHeader title="Activity" description="Recent events across your workspace.">
          <PageSection title="Today">
            <GroupContainer>
              <GroupRow title="Deployed API" value="09:24" />
              <GroupRow title="Invited Sam Rivera" value="10:03" />
            </GroupContainer>
          </PageSection>
        </PageHeader>
      </AppLayout>
    </div>
  )
};
