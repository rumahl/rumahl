import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  AppLayout,
  AppNavLayout,
  AppSidebar,
  GroupContainer,
  GroupRow,
  Navbar,
  NavbarItem,
  PageHeader,
  PageSection,
  RumahlMark,
  SidenavItem,
  SidenavSection
} from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, PulseIcon, SearchIcon, SettingsIcon, SparkIcon } from "@rumahl/ui/icons";

const NAV = [
  { id: "discover", label: "Discover", icon: <SparkIcon /> },
  { id: "apps", label: "Apps", icon: <GridIcon /> },
  { id: "activity", label: "Activity", icon: <PulseIcon /> },
  { id: "settings", label: "Settings", icon: <SettingsIcon /> }
];

const meta = {
  title: "Layout/App/Navbar + sidenav",
  component: AppNavLayout,
  args: { children: null },
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: `The standard navbar + sidenav application frame.

- The desktop rail is a \`Sidenav\` / \`AppSidebar\`; the navbar slot is optional when the host puts navigation in a window titlebar.
- Below **760px** the rail is replaced by the standard \`MobileIslandNav\` supplied through \`mobileNav\`.
- The island's active pill animates on tap and is draggable; the selection commits on release. Arrow keys move between destinations.
- Keep the island destinations to one level and mirror the rail's primary sections only.

## Rules
- Use this layout for app shells; do not re-create the rail/island breakpoint per app.
- \`mobileNav\` is required for an app that needs mobile navigation.
- Navbar, rail and island all use the same destination ids.`
      }
    }
  }
} satisfies Meta<typeof AppNavLayout>;
export default meta;
type Story = StoryObj<typeof meta>;

function Demo({ compact = false }: { compact?: boolean }) {
  const [page, setPage] = useState("discover");
  const content = <PageHeader title={NAV.find((item) => item.id === page)?.label ?? "Discover"} description="A standard app frame with desktop rail and mobile island navigation.">
    <PageSection title="Overview">
      <GroupContainer>
        <GroupRow title="Layout" value="Navbar + sidenav" />
        <GroupRow title="Mobile" value="Draggable island" />
        <GroupRow title="Breakpoint" value="760px" />
      </GroupContainer>
    </PageSection>
  </PageHeader>;
  return <div style={{ width: compact ? 390 : undefined, height: compact ? 700 : 620 }}>
    <AppNavLayout
      navbar={<Navbar brand={<><RumahlMark style={{ width: 14, height: 24 }} /> App</>} actions={<SearchIcon />} label="App">
        {NAV.slice(0, 3).map((item) => <NavbarItem key={item.id} as="button" type="button" icon={item.icon} current={page === item.id} onClick={() => setPage(item.id)}>{item.label}</NavbarItem>)}
      </Navbar>}
      sidebar={<AppSidebar>
        <SidenavSection title="Browse">
          {NAV.map((item) => <SidenavItem key={item.id} as="button" type="button" icon={item.icon} current={page === item.id} onClick={() => setPage(item.id)}>{item.label}</SidenavItem>)}
        </SidenavSection>
      </AppSidebar>}
      mobileNav={{
        label: "App navigation",
        items: NAV,
        value: page,
        onChange: setPage
      }}
    >
      {content}
    </AppNavLayout>
  </div>;
}

export const Desktop: Story = {
  parameters: { docs: { source: { language: "tsx", code: `import { AppNavLayout } from "@rumahl/ui/components";\n\n<AppNavLayout sidebar={<AppSidebar>…</AppSidebar>} mobileNav={{ items, value, onChange, label: "App navigation" }}>\n  …\n</AppNavLayout>` } } },
  render: () => <Demo />
};

export const Mobile: Story = {
  render: () => <Demo compact />
};

/** The plain layout primitive remains available for Settings-style panes. */
export const ClassicSidebarLayout: Story = {
  render: () => <div style={{ height: 460 }}><AppLayout sidebar={<AppSidebar><SidenavSection><SidenavItem icon={<DesktopIcon />} current>Overview</SidenavItem></SidenavSection></AppSidebar>}><PageHeader title="Overview" description="Uses the older two-pane layout without the mobile island." /></AppLayout></div>
};
