import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  GroupContainer,
  GroupLink,
  GroupLinkContainer,
  GroupRow,
  RumahlBadge,
  RumahlSwitch
} from "@rumahl/ui/components";
import { DesktopIcon, DocumentIcon, GridIcon, InfoIcon } from "@rumahl/ui/icons";

/**
 * # Group
 *
 * Native grouping containers. `GroupContainer` holds label rows
 * (`GroupRow`); `GroupLinkContainer` holds drill-in links (`GroupLink`).
 * These are the building blocks behind every settings-style screen.
 */
const meta = {
  title: "Components/Group",
  component: GroupContainer,
  args: { children: null },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The native row-grouping container (settings, preferences, app panes).

- \`GroupContainer\` holds \`GroupRow\`s (label + control/value).
- \`GroupLinkContainer\` holds drill-in \`GroupLink\` rows.

## Rules
- The group is a subtle **\`--row\` surface**, not a glass card — it sits on the
  window / content surface.
- **One topic per group**; keep rows scannable (short title, optional muted
  subtitle).
- Values are muted and end-aligned; controls sit on the end.`
      }
    }
  }
} satisfies Meta<typeof GroupContainer>;
export default meta;
type Story = StoryObj<typeof meta>;

/** Labeled rows with a control or a read-only value on the right. */
export const Container: Story = {
  render: () => (
    <div style={{ width: 560 }}>
      <GroupContainer>
        <GroupRow title="Dark mode" description="Use a darker colour scheme.">
          <RumahlSwitch label="Dark mode" defaultChecked />
        </GroupRow>
        <GroupRow title="Theme" value="Default" />
        <GroupRow title="Update" value={<RumahlBadge tone="success">Up to date</RumahlBadge>} />
      </GroupContainer>
    </div>
  )
};

/** Drill-in links: a chevron for internal, a glyph for external. */
export const LinkContainer: Story = {
  render: () => (
    <div style={{ width: 560 }}>
      <GroupLinkContainer>
        <GroupLink
          icon={<DesktopIcon />}
          title="Personalization"
          subtitle="Theme, wallpaper and accent colour"
          href="#personalization"
        />
        <GroupLink icon={<GridIcon />} title="Workspace" subtitle="Apps and layout" href="#workspace" />
        <GroupLink icon={<InfoIcon />} title="Website" subtitle="rumahl.dev" external href="https://rumahl.dev" />
        <GroupLink icon={<DocumentIcon />} title="Source" subtitle="github.com/rumahl/rumahl" external href="https://github.com/rumahl/rumahl" />
      </GroupLinkContainer>
    </div>
  )
};
