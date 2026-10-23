import { useRef, useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { AnchoredPopover, Menu, MenuDivider, MenuItem } from "@rumahl/ui/components";

const meta = {
  title: "Components/AnchoredPopover",
  component: AnchoredPopover,
  args: { anchor: { current: null }, open: false, children: null },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A popover positioned relative to an anchor element.

## Rules
- Anchored to its trigger; flips and clamps to stay on screen.
- Glass panel, elevated; closes on outside click / Escape.
- For action lists use **Menu**; for coach-marks use **Tipseen**.`
      }
    }
  }
} satisfies Meta<typeof AnchoredPopover>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const anchor = useRef<HTMLButtonElement>(null);
    const [open, setOpen] = useState(false);
    return (
      <div>
        <button ref={anchor} type="button" className="btn btn--secondary" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
          Options ▾
        </button>
        <AnchoredPopover anchor={anchor} open={open}>
          <Menu>
            <MenuItem shortcut="⌘R">Rename</MenuItem>
            <MenuItem shortcut="⌘D">Duplicate</MenuItem>
            <MenuDivider />
            <MenuItem>Delete</MenuItem>
          </Menu>
        </AnchoredPopover>
      </div>
    );
  }
};
