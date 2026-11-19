import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ToggleTile } from "@rumahl/ui/components";
import { BoltIcon, MoonIcon, ShieldIcon } from "@rumahl/ui/icons";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/ToggleTile",
  component: ToggleTile,
  args: { label: "Dark mode", active: false },
  argTypes: { ...nativeArgTypes, active: { control: "boolean" }, color: { control: "color" } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A large quick-setting tile (control-center style).

## Rules
- For **on/off quick settings** with an icon and a noun label.
- \`active\` is the on state; keep the label a noun.`
      }
    }
  }
} satisfies Meta<typeof ToggleTile>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => <ToggleTile {...args} icon={<MoonIcon />} onClick={() => {}} />
};

export const ControlCenter: Story = {
  render: () => {
    const [state, setState] = useState<Record<string, boolean>>({ dark: true, animations: true, vpn: false });
    const tiles = [
      { id: "dark", label: "Dark mode", icon: <MoonIcon /> },
      { id: "animations", label: "Animations", icon: <BoltIcon /> },
      { id: "vpn", label: "Protection", icon: <ShieldIcon />, description: "Always on" }
    ];
    return (
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, width: 300 }}>
        {tiles.map((tile) => (
          <ToggleTile
            key={tile.id}
            label={tile.label}
            icon={tile.icon}
            {...(tile.description ? { description: tile.description } : {})}
            active={state[tile.id]}
            onClick={() => setState((current) => ({ ...current, [tile.id]: !current[tile.id] }))}
          />
        ))}
      </div>
    );
  }
};
