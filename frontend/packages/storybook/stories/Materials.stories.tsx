import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Button,
  RumahlBadge,
  RumahlInput,
  RumahlNotice,
  RumahlSelect,
  RumahlSwitch
} from "@rumahl/ui/components";

/**
 * # Materials
 *
 * The rumahl material system has three states. They are **element states**, not
 * themes: the same component renders each one.
 *
 * - **Glass** — recessed glass. Inputs and buttons sit *inside* a surface; the
 *   depth comes from an inset recess and a backdrop blur. This is the default
 *   and the desktop look.
 * - **Solid** — flat. No recess, no blur and no depth; a plain filled surface.
 *   Chosen per theme for the app input fields (Glass/Solid) and offered as a
 *   whole-shell material.
 * - **Floating** — liquid glass. For controls that float *above* a picture or
 *   content (the launcher, toolbars that follow scroll). Floating is never
 *   recessed; the specular highlight sits on top of a translucent fill so even
 *   the coloured variants read as glass.
 *
 * Use the **Material** toolbar to switch all stories between the three.
 *
 * ## Rules
 *
 * - **Glass is the default.** Everything recessed lives on a glass surface;
 *   reach for solid only for the flat desktop look or reduced-transparency.
 * - **Reserved for chrome, never for content.** Sidebars, toolbars, the window
 *   frame, menus, the dock and popovers carry the material. Content areas are
 *   plain — see *Design System / Surfaces*.
 * - **`floating` is for controls only, over a picture or another surface.**
 *   Never nest a floating element inside a recessed one. A single floating
 *   control is its own round island; use a bar only for adjacent controls.
 * - **Floating reuses the shell material**, so dock / menu-bar / floating
 *   controls stay consistent (`Glass → Transparenz → Solid`).
 * - **Reduced transparency wins.** `prefers-reduced-transparency: reduce` turns
 *   every glass surface into an opaque `surface-strong` surface, and every
 *   material becomes solid. The top bar is deliberately excluded from the system.
 */
const meta = {
  title: "Foundations/Materials",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component:
          "Glass = recessed glass, Solid = flat, Floating = liquid glass over " +
          "content. Switch the Material toolbar to compare. Inputs and buttons " +
          "share the same recipe; only the tint (colour) and the material differ."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Kit() {
  const [value, setValue] = useState("auto");
  const [on, setOn] = useState(true);
  return (
    <div style={{ display: "grid", gap: 18, width: 460 }}>
      <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
        <Button variant="secondary">Secondary</Button>
        <Button variant="primary">Primary</Button>
        <Button variant="danger">Danger</Button>
        <Button variant="ghost">Ghost</Button>
      </div>
      <RumahlInput placeholder="Text field" defaultValue="Input" />
      <RumahlSelect
        label="Select"
        value={value}
        onChange={setValue}
        options={[
          { value: "auto", label: "Automatic" },
          { value: "light", label: "Light" },
          { value: "dark", label: "Dark" }
        ]}
      />
      <RumahlSwitch label="Switch" checked={on} onChange={(event) => setOn(event.target.checked)} />
      <div style={{ display: "flex", gap: 8 }}>
        <RumahlBadge tone="success">Success</RumahlBadge>
        <RumahlBadge tone="warning">Warning</RumahlBadge>
      </div>
      <RumahlNotice title="Notice" tone="info">
        A notice uses the same material as the controls.
      </RumahlNotice>
    </div>
  );
}

export const Overview: Story = {
  render: () => <Kit />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { useState } from "react";
import {
  Button,
  RumahlBadge,
  RumahlInput,
  RumahlNotice,
  RumahlSelect,
  RumahlSwitch
} from "@rumahl/ui/components";

export function Example() {
  const [value, setValue] = useState("auto");
  const [on, setOn] = useState(true);
  return (
    <div style={{ display: "grid", gap: 18, width: 460 }}>
      <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
        <Button variant="secondary">Secondary</Button>
        <Button variant="primary">Primary</Button>
        <Button variant="danger">Danger</Button>
        <Button variant="ghost">Ghost</Button>
      </div>

      <RumahlInput placeholder="Text field" defaultValue="Input" />

      <RumahlSelect
        label="Select"
        value={value}
        onChange={setValue}
        options={[
          { value: "auto", label: "Automatic" },
          { value: "light", label: "Light" },
          { value: "dark", label: "Dark" }
        ]}
      />

      <RumahlSwitch label="Switch" checked={on} onChange={(event) => setOn(event.target.checked)} />

      <div style={{ display: "flex", gap: 8 }}>
        <RumahlBadge tone="success">Success</RumahlBadge>
        <RumahlBadge tone="warning">Warning</RumahlBadge>
      </div>

      <RumahlNotice title="Notice" tone="info">
        A notice uses the same material as the controls.
      </RumahlNotice>
    </div>
  );
}`
      }
    }
  }
};
