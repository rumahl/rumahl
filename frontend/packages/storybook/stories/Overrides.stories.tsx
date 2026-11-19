import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlInput, RumahlInputGroup } from "@rumahl/ui/components";
import { SearchIcon } from "@rumahl/ui/icons";

/**
 * Flexibility of the rumahl/ui building blocks.
 *
 * Every component accepts the native element props (so `className` and `style`
 * can be extended) plus a few override props:
 *
 * - `glass={false}` — flat control without glass (override).
 * - `floating` — element state for controls that hover over a background.
 * - `variant` / `size` — visual variants where applicable.
 *
 * This is the escape hatch for building something of your own on top of
 * rumahl/ui without writing a full custom theme.
 */
const meta = {
  title: "Guides/Overrides",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component:
          "More props, custom className/style and overrides like glass={false} " +
          "or floating — an alternative to rebuilding everything in a custom theme."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Buttons: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
      <Button>Default glass</Button>
      <Button glass={false}>glass={false} (flat)</Button>
      <Button variant="primary">Primary</Button>
      <Button variant="primary" glass={false}>Primary flat</Button>
      <Button floating>floating</Button>
      {/* Custom className / style are just native props. */}
      <Button style={{ borderRadius: 12, paddingInline: 22 }} className="my-custom-button">
        custom style
      </Button>
    </div>
  )
};

export const Inputs: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 14, width: 420 }}>
      <RumahlInput placeholder="Default glass" />
      <RumahlInput glass={false} placeholder="glass={false} (flat)" />
      <RumahlInput floating placeholder="floating" />
      <RumahlInputGroup prefix={<SearchIcon aria-hidden="true" />} placeholder="InputGroup glass" />
      <RumahlInputGroup glass={false} prefix={<SearchIcon aria-hidden="true" />} placeholder="InputGroup flat" />
      <RumahlInputGroup floating prefix={<SearchIcon aria-hidden="true" />} placeholder="InputGroup floating" />
      <RumahlInput style={{ borderColor: "#e5484d" }} placeholder="custom style" />
    </div>
  )
};
