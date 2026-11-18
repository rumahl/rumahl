import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  CommandPalette,
  CommandPaletteItem,
  CommandPaletteSection,
  CommandPaletteSeparator
} from "@rumahl/ui/components";
import { DocumentIcon, GridIcon, SearchIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Command-center app layout
 *
 * A template/inspiration for an overlay: a dimmed backdrop with the rumahl OS
 * `CommandPalette` centred on top (search + results). Layer it over any other
 * layout for a spotlight / ⌘K surface.
 */
const meta = {
  title: "Layout/App/Command center",
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [query, setQuery] = useState("cal");
    return (
      <div
        style={{
          position: "relative",
          height: 500,
          overflow: "hidden",
          background:
            "radial-gradient(ellipse at 70% 10%, #79b6ae66, transparent 60%), linear-gradient(135deg, #2b6cb0, #1f6f5c)"
        }}
      >
        <div style={{ position: "absolute", inset: 0, background: "#0b0d1240", backdropFilter: "blur(2px)" }} />
        <div style={{ position: "relative", display: "flex", justifyContent: "center", paddingTop: 90 }}>
          <CommandPalette
            value={query}
            onChange={setQuery}
            label="Command palette"
            placeholder="Search commands, apps and files"
            hint="Esc"
            style={{ width: 620, maxWidth: "92%" }}
          >
            <CommandPaletteSection title="Commands">
              <CommandPaletteItem icon={<SearchIcon />} shortcut="↵" selected>
                Search for “{query}”
              </CommandPaletteItem>
            </CommandPaletteSection>
            <CommandPaletteSeparator />
            <CommandPaletteSection title="Apps">
              <CommandPaletteItem icon={<GridIcon />} shortcut="⌘1">
                Calendar
              </CommandPaletteItem>
              <CommandPaletteItem icon={<DocumentIcon />} shortcut="⌘2">
                Calculator
              </CommandPaletteItem>
              <CommandPaletteItem icon={<SettingsIcon />} shortcut="⌘,">
                Open settings
              </CommandPaletteItem>
            </CommandPaletteSection>
          </CommandPalette>
        </div>
      </div>
    );
  }
};
