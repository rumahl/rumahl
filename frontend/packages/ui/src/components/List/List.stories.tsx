import type { Meta, StoryObj } from "@storybook/react-vite";
import { List, ListItem, RumahlAvatar, RumahlBadge } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const LIST_DOCS = `A vertical list of items with leading / trailing slots.

## Rules
- \`ListItem\`: leading (icon/avatar), title, subtitle, trailing (badge/action).
- One primary line per row; secondary info is muted.
- Rows are tappable only when they lead somewhere — show a chevron then.`;

const meta = {
  title: "Components/List",
  argTypes: nativeArgTypes,
  parameters: { layout: "centered", docs: { description: { component: LIST_DOCS } } }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div style={{ width: 360 }}>
      <List>
        <ListItem title="Alex Morgan" subtitle="alex@rumahl.dev" leading={<RumahlAvatar name="Alex Morgan" size="sm" />} trailing={<RumahlBadge tone="success">Active</RumahlBadge>} />
        <ListItem title="Sam Rivera" subtitle="sam@rumahl.dev" leading={<RumahlAvatar name="Sam Rivera" size="sm" />} />
        <ListItem title="Storage" subtitle="128 GB of 512 GB used" trailing={<RumahlBadge tone="warning">75%</RumahlBadge>} />
      </List>
    </div>
  )
};

export const Interactive: Story = {
  render: () => (
    <div style={{ width: 360 }}>
      <List>
        <ListItem interactive title="Display" subtitle="Resolution, scaling and brightness" onClick={() => {}} />
        <ListItem interactive title="Sound" subtitle="Output device and volume" onClick={() => {}} />
      </List>
    </div>
  )
};
