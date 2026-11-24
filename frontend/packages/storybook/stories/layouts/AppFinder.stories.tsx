import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Box,
  Flex,
  Heading,
  IconButton,
  RumahlInput,
  SidenavItem,
  SidenavSection,
  Sidenav,
  Text,
  Toolbar,
  ToolbarGroup,
  ToolbarSpacer
} from "@rumahl/ui/components";
import { ArrowIcon, DesktopIcon, DocumentIcon, GridIcon, HomeIcon, ArrowLeftIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Finder / explorer (app style)
 *
 * A template/inspiration: a location sidebar, a toolbar and a content grid.
 * Content on the surface; material for the sidebar and toolbar.
 */
const meta = { title: "Layout/App/Finder", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const PLACES = [
  { id: "home", label: "Home", icon: <HomeIcon />, current: true },
  { id: "desktop", label: "Desktop", icon: <DesktopIcon /> },
  { id: "documents", label: "Documents", icon: <DocumentIcon /> },
  { id: "apps", label: "Applications", icon: <GridIcon /> }
];

const FILES = ["Invoices", "Photos", "Projects", "Notes", "Backups", "Exports", "Screenshots", "Downloads"];

export const Default: Story = {
  render: () => (
    <div style={{ height: 560, display: "grid", gridTemplateColumns: "220px minmax(0, 1fr)" }}>
      <Sidenav>
        <SidenavSection title="Places">
          {PLACES.map((place) => (
            <SidenavItem key={place.id} as="a" href="#" icon={place.icon} current={place.current === true}>
              {place.label}
            </SidenavItem>
          ))}
        </SidenavSection>
      </Sidenav>

      <Flex direction="column" style={{ minWidth: 0 }}>
        <div style={{ padding: "12px 16px" }}>
          <Toolbar>
            <ToolbarGroup>
              <IconButton label="Back">
                <ArrowLeftIcon />
              </IconButton>
              <IconButton label="Forward">
                <ArrowIcon />
              </IconButton>
            </ToolbarGroup>
            <ToolbarSpacer />
            <div style={{ width: 220 }}>
              <RumahlInput type="search" placeholder="Search" aria-label="Search" />
            </div>
            <ToolbarGroup>
              <IconButton label="Settings">
                <SettingsIcon />
              </IconButton>
            </ToolbarGroup>
          </Toolbar>
        </div>

        <div style={{ padding: "8px 20px 0" }}>
          <Heading level={5}>Documents</Heading>
        </div>

        <div style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))", gap: 14, overflow: "auto" }}>
          {FILES.map((file) => (
            <Box key={file} p={3} radius="14px" style={{ display: "grid", justifyItems: "center", gap: 8, cursor: "pointer" }}>
              <span style={{ display: "grid", placeItems: "center", width: 48, height: 48, borderRadius: 12, color: "var(--rumahl-ui-color-accent)", background: "color-mix(in srgb, var(--rumahl-ui-color-accent) 14%, transparent)" }}>
                <DocumentIcon />
              </span>
              <Text size="sm">{file}</Text>
            </Box>
          ))}
        </div>
      </Flex>
    </div>
  )
};
