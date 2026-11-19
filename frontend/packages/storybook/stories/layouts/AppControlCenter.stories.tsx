import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { GroupContainer, GroupRow, Heading, RumahlRange, ToggleTile } from "@rumahl/ui/components";
import { BoltIcon, MoonIcon, SearchIcon, ShieldIcon } from "@rumahl/ui/icons";

/**
 * # Control centre (app style)
 *
 * A template/inspiration: a grid of quick toggles plus sliders. Material marks
 * the tiles; the rest sits on the surface.
 */
const meta = { title: "Layout/App/Control center", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [state, setState] = useState<Record<string, boolean>>({ dark: true, performance: false, vpn: true });
    const toggle = (id: string) => setState((current) => ({ ...current, [id]: !current[id] }));
    return (
      <div style={{ height: 560, padding: 24, display: "grid", alignContent: "start", gap: 16, maxWidth: 440 }}>
        <Heading level={5}>Control centre</Heading>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
          <ToggleTile label="Dark mode" icon={<MoonIcon />} active={state.dark} onClick={() => toggle("dark")} />
          <ToggleTile label="Performance" icon={<BoltIcon />} active={state.performance} onClick={() => toggle("performance")} />
          <ToggleTile label="Protection" icon={<ShieldIcon />} description="Always on" active={state.vpn} onClick={() => toggle("vpn")} />
          <ToggleTile label="Focus" icon={<SearchIcon />} onClick={() => toggle("focus")} />
        </div>
        <GroupContainer>
          <GroupRow title="Brightness">
            <div style={{ width: 170 }}>
              <RumahlRange label="Brightness" defaultValue={70} />
            </div>
          </GroupRow>
          <GroupRow title="Volume">
            <div style={{ width: 170 }}>
              <RumahlRange label="Volume" defaultValue={40} />
            </div>
          </GroupRow>
        </GroupContainer>
      </div>
    );
  }
};
