import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  GroupContainer,
  GroupRow,
  List,
  ListItem,
  PageHeader,
  PageSection,
  RumahlBadge,
  RumahlProgress,
  Text
} from "@rumahl/ui/components";

/**
 * # Island-based app layout
 *
 * A template/inspiration for a dashboard: a few floating “islands” (widgets)
 * for the key numbers, with the main content sitting directly on the work
 * surface. Material marks the islands; everything else is plain.
 */
const meta = {
  title: "Layout/App/Island dashboard",
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div style={{ height: 520, padding: 28, overflow: "auto" }}>
      <PageHeader title="Overview" description="Material islands for the key numbers; the rest lives on the surface." />

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
          gap: 16,
          margin: "8px 0 26px"
        }}
      >
        <GroupContainer>
          <GroupRow title="Deploys" value="128" />
          <div style={{ padding: "0 16px 16px" }}>
            <RumahlProgress size="sm" label="Deploys" value={72} />
          </div>
        </GroupContainer>
        <GroupContainer>
          <GroupRow title="Open issues" value={<RumahlBadge tone="warning">7</RumahlBadge>} />
          <GroupRow title="Closed this week" value="23" />
        </GroupContainer>
        <GroupContainer>
          <GroupRow title="Uptime" value={<RumahlBadge tone="success">99.98%</RumahlBadge>} />
          <GroupRow title="Response" value="180 ms" />
        </GroupContainer>
      </div>

      <PageSection title="Recent activity">
        <List>
          <ListItem title="Deployed api@2.4.0" subtitle="5 minutes ago" />
          <ListItem title="Opened issue #412: retry on timeout" subtitle="1 hour ago" />
          <ListItem title="Merged “split layout template”" subtitle="Yesterday" />
        </List>
      </PageSection>

      <Text tone="muted" size="sm" style={{ display: "block", marginTop: 18 }}>
        Islands can be rearranged or dropped freely — they are just grouped components.
      </Text>
    </div>
  )
};
