import type { Meta, StoryObj } from "@storybook/react-vite";
import { Tray, TrayItem } from "@rumahl/ui/components";
import { BoltIcon, MoonIcon, SearchIcon, ShieldIcon } from "@rumahl/ui/icons";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Tray",
  component: Tray,
  args: { children: null },
  argTypes: nativeArgTypes,
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The status-item cluster (the menu-bar right island).

## Rules
- **System status only**; keep the icons few and interactive.
- \`TrayItem\` needs a \`label\`; \`active\` marks a toggled state.`
      }
    }
  }
} satisfies Meta<typeof Tray>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <Tray>
      <TrayItem label="Search">
        <SearchIcon />
      </TrayItem>
      <TrayItem label="Focus" active>
        <MoonIcon />
      </TrayItem>
      <TrayItem label="Power">
        <BoltIcon />
      </TrayItem>
      <TrayItem label="Privacy">
        <ShieldIcon />
      </TrayItem>
    </Tray>
  )
};
