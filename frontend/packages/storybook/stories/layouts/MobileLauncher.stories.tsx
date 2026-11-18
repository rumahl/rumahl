import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { AppGrid, AppTile, IconButton, PhoneFrame, RumahlSearch } from "@rumahl/ui/components";
import { BoltIcon, HomeIcon, SearchIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Mobile launcher (hybrid template)
 *
 * A **presentational** template for a phone home screen: a search field and an
 * app grid inside a `PhoneFrame`. It composes rumahl/ui building blocks only.
 *
 * This is a layout *template*, not the OS launcher: the real launcher behaviour
 * (app catalog, folders, drag, opening apps) lives in the shell. rumahl OS opens
 * on a phone inside the launcher.
 */
const meta = { title: "Layout/Mobile/Launcher", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const APPS = [
  { id: "files", title: "Files" },
  { id: "photos", title: "Photos" },
  { id: "calendar", title: "Calendar" },
  { id: "notes", title: "Notes" },
  { id: "mail", title: "Mail" },
  { id: "music", title: "Music" },
  { id: "maps", title: "Maps" },
  { id: "settings", title: "Settings", system: true }
];

export const Default: Story = {
  render: () => {
    const [query, setQuery] = useState("");
    return (
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
              <IconButton shape="circle" variant="primary" label="Home">
                <HomeIcon />
              </IconButton>
              <IconButton shape="circle" variant="ghost" label="Search">
                <SearchIcon />
              </IconButton>
              <IconButton shape="circle" variant="ghost" label="Activity">
                <BoltIcon />
              </IconButton>
              <IconButton shape="circle" variant="ghost" label="Settings">
                <SettingsIcon />
              </IconButton>
            </div>
          }
        >
          <div style={{ padding: "10px 16px 18px", display: "grid", gap: 18 }}>
            <RumahlSearch label="Search apps" clearLabel="Clear" value={query} onChange={setQuery} />
            <AppGrid columns={3}>
              {APPS.filter((app) => app.title.toLowerCase().includes(query.toLowerCase())).map((app) => (
                <AppTile key={app.id} id={app.id} title={app.title} system={app.system === true} onClick={() => {}} />
              ))}
            </AppGrid>
          </div>
        </PhoneFrame>
      </div>
    );
  }
};
