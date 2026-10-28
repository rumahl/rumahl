import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { MobileIslandNav } from "@rumahl/ui/components";
import { GridIcon, SearchIcon, SettingsIcon, SparkIcon } from "@rumahl/ui/icons";

const ITEMS = [
  { id: "discover", label: "Discover", icon: <SparkIcon /> },
  { id: "apps", label: "Apps", icon: <GridIcon /> },
  { id: "search", label: "Search", icon: <SearchIcon /> },
  { id: "settings", label: "Settings", icon: <SettingsIcon /> }
];

const meta = {
  title: "Components/Mobile island",
  component: MobileIslandNav,
  args: { items: ITEMS, value: "discover", onChange: () => {}, label: "App navigation" },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The standard narrow-window navigation island.

## Rules
- **One level, 4–6 destinations.** Mirror the desktop rail's primary sections.
- The active indicator is the navigation control: tap a destination and the pill animates into place; drag it across items to preview a destination.
- Selection commits on release; use Arrow keys / Home / End for keyboard navigation.
- All motion follows the shell animation setting / reduced-motion preference.
- Apps get this automatically through \`AppNavLayout\`'s \`mobileNav\` below 760px — do not hand-roll a second mobile bar.

The island is dark glass by default and follows the \`--island-*\` recipe variables; the clean base turns it into a square, light bar.`
      }
    }
  }
} satisfies Meta<typeof MobileIslandNav>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [value, setValue] = useState("discover");
    return <div style={{ width: 390, padding: 24 }}>
      <MobileIslandNav items={ITEMS} value={value} onChange={setValue} label="App navigation" />
    </div>;
  }
};

export const Narrow: Story = {
  render: () => {
    const [value, setValue] = useState("apps");
    return <div style={{ width: 300, padding: 12 }}>
      <MobileIslandNav items={ITEMS} value={value} onChange={setValue} label="App navigation" />
    </div>;
  }
};
