import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Menu, MenuDivider, MenuItem } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const MENU_DOCS = `A compact list of actions or options (popovers, context menus, dropdowns).

- \`MenuDivider\` separates groups; items may carry icons, right-aligned
  shortcuts, or \`danger\`.
- The panel is glass; rows are 30px tall with a 16px icon column reserved **only
  when any item has an icon**.

## Rules
- **One verb per item**; the destructive action goes last and uses \`danger\`.
- **Group related items** — never mix navigation and actions in one menu.
- **Keep it short**; deeper structure belongs in a dialog or a submenu.`;

const meta = {
  title: "Components/Menu",
  argTypes: nativeArgTypes,
  parameters: { layout: "centered", docs: { description: { component: MENU_DOCS } } }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <Menu>
      <MenuItem shortcut="⌘N">New window</MenuItem>
      <MenuItem shortcut="⌘O">Open…</MenuItem>
      <MenuDivider />
      <MenuItem>Rename</MenuItem>
      <MenuItem disabled>Move to trash</MenuItem>
    </Menu>
  )
};

export const Checkable: Story = {
  render: () => {
    const [on, setOn] = useState(true);
    return (
      <Menu>
        <MenuItem selected={on} onClick={() => setOn((value) => !value)}>Show hidden files</MenuItem>
        <MenuItem selected={false}>Sort by name</MenuItem>
      </Menu>
    );
  }
};
