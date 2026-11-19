import type { Meta, StoryObj } from "@storybook/react-vite";
import { MenuBar, TrayItem } from "@rumahl/ui/components";
import { MoonIcon, SearchIcon, ShieldIcon } from "@rumahl/ui/icons";

/**
 * # Menu bar
 *
 * Protected system chrome: the rumahl OS top bar as two floating islands
 * (brand + menus, status + clock). Pure CSS on the island recipe, so it looks
 * the same in the OS, apps and Storybook.
 */
const MENUS = [
  { id: "file", label: "File" },
  { id: "edit", label: "Edit" },
  { id: "view", label: "View" },
  { id: "window", label: "Window" }
];

const meta = {
  title: "System/MenuBar",
  component: MenuBar,
  args: { brand: "rumahl", menus: MENUS, clock: "9:41 AM", onSelect: () => {} },
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: `The top system bar (menus on the left, status / clock on the right).

## Rules
- **System chrome only** — apps never render it.
- Menus open as dropdowns; the bar floats over the wallpaper.`
      }
    }
  }
} satisfies Meta<typeof MenuBar>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <div style={{ padding: 16 }}>
      <MenuBar
        {...args}
        status={
          <>
            <TrayItem label="Search">
              <SearchIcon />
            </TrayItem>
            <TrayItem label="Focus">
              <MoonIcon />
            </TrayItem>
            <TrayItem label="Privacy">
              <ShieldIcon />
            </TrayItem>
          </>
        }
      />
    </div>
  )
};
