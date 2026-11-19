import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  GroupContainer,
  GroupRow,
  Heading,
  List,
  ListItem,
  PageHeader,
  RumahlAvatar,
  RumahlBadge,
  Text
} from "@rumahl/ui/components";

/**
 * # Split-inspector app layout
 *
 * A template/inspiration for master–detail (mail, files, people): a list, a
 * content column and an inspector, separated by hairlines. The content sits on
 * the work surface; material is used for the controls inside.
 */
const meta = {
  title: "Layout/App/Split inspector",
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const MAIL = [
  { id: "a", from: "Alex Morgan", subject: "Quarterly plan", time: "09:24" },
  { id: "b", from: "Sam Rivera", subject: "Design review notes", time: "08:10" },
  { id: "c", from: "Jo Chen", subject: "Deploy checklist", time: "Mon" },
  { id: "d", from: "Dana Roy", subject: "Welcome aboard", time: "Mon" }
];

export const Default: Story = {
  render: () => (
    <div style={{ height: 520, display: "grid", gridTemplateColumns: "280px minmax(0, 1fr) 260px" }}>
      <div style={{ overflow: "auto", borderRight: "1px solid var(--surface-edge)" }}>
        <List>
          {MAIL.map((mail) => (
            <ListItem
              key={mail.id}
              interactive
              leading={<RumahlAvatar name={mail.from} size="sm" />}
              title={mail.from}
              subtitle={mail.subject}
              trailing={<Text tone="muted" size="sm" as="span">{mail.time}</Text>}
            />
          ))}
        </List>
      </div>

      <div style={{ overflow: "auto", padding: "26px 30px" }}>
        <PageHeader title="Quarterly plan" description="From Alex Morgan · alex@rumahl.dev" />
        <Text as="p" style={{ maxWidth: 560, lineHeight: 1.65 }}>
          Here is the plan for the next quarter. We keep the scope tight and ship the
          layout templates first, then refine the controls. Reply with your notes and
          I will fold them into the roadmap.
        </Text>
        <Text as="p" tone="muted" style={{ maxWidth: 560, marginTop: 16, lineHeight: 1.65 }}>
          Everything in this view is composed from rumahl/ui components; the columns are
          just plain surface with hairline dividers.
        </Text>
      </div>

      <div style={{ overflow: "auto", padding: 18, borderLeft: "1px solid var(--surface-edge)" }}>
        <Heading level={6} style={{ marginBottom: 10 }}>
          Inspector
        </Heading>
        <GroupContainer>
          <GroupRow title="Priority" value={<RumahlBadge tone="warning">High</RumahlBadge>} />
          <GroupRow title="Labels" value="Roadmap" />
          <GroupRow title="Due" value="Fri" />
        </GroupContainer>
      </div>
    </div>
  )
};
