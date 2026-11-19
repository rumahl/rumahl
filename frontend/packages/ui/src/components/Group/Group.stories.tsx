import type { Meta, StoryObj } from "@storybook/react-vite";
import { useId, useState } from "react";
import {
  GroupContainer,
  GroupFooter,
  Button,
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

function FieldGroupsExample({ narrow = false }: { narrow?: boolean }) {
  const id = useId();
  const switchId = useId();
  const [enabled, setEnabled] = useState(true);
  const [location, setLocation] = useState("desktop");
  return <div style={{ width: narrow ? 320 : 620, maxWidth: "100%" }}>
    <h3>Notifications</h3>
    <GroupContainer>
      <GroupRow title="Show notifications" description="Keep updates within reach without interrupting your work." controlId={switchId}>
        <input id={switchId} type="checkbox" role="switch" checked={enabled} onChange={event => setEnabled(event.target.checked)} />
      </GroupRow>
      <GroupRow title="Delivery" description="Choose where updates appear." controlId={id}>
        <select id={id} className="rumahl-input" value={location} disabled={!enabled} onChange={event => setLocation(event.target.value)}>
          <option value="desktop">Desktop</option><option value="center">Notification center</option>
        </select>
      </GroupRow>
      <GroupRow title="Storage" value="This browser" />
      <GroupFooter description="Only this group is reset; other preferences stay unchanged.">
        <Button size="sm" disabled={enabled && location === "desktop"} onClick={() => { setEnabled(true); setLocation("desktop"); }}>Reset group</Button>
      </GroupFooter>
    </GroupContainer>
  </div>;
}

/** Interactive field group; aligned controls and one shared inset divider. */
export const FieldGroups: Story = { render: () => <FieldGroupsExample /> };
/** Container-based wrapping; switches stay beside their label. */
export const NarrowFieldGroups: Story = { render: () => <FieldGroupsExample narrow /> };
