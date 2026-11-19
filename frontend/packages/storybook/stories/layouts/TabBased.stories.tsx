import { useState, type CSSProperties } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  GroupContainer,
  GroupRow,
  Heading,
  List,
  ListItem,
  PageHeader,
  RumahlBadge,
  RumahlNotice,
  RumahlSwitch,
  RumahlTabs,
  Text
} from "@rumahl/ui/components";

/**
 * # Tab-based app layout
 *
 * A template/inspiration for a document app: one header, sections switched in
 * place with `RumahlTabs`. The document sits directly on the work surface —
 * material is reserved for navigation and controls.
 */
const meta = {
  title: "Layout/App/Tab layout",
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const PANEL: CSSProperties = { padding: "20px 0 0" };

export const Default: Story = {
  render: () => {
    const [tab, setTab] = useState("document");
    return (
      <div style={{ height: 540, display: "flex", flexDirection: "column" }}>
        <div style={{ padding: "22px 30px 0" }}>
          <PageHeader title="Project Alpha" description="Tabs switch sections in place; content sits on the surface." />
        </div>
        <div style={{ padding: "16px 30px 0", flex: 1, overflow: "auto" }}>
          <RumahlTabs
            label="Sections"
            value={tab}
            onChange={setTab}
            tabs={[
              {
                id: "document",
                label: "Document",
                content: (
                  <article style={{ ...PANEL, maxWidth: 660, display: "grid", gap: 14 }}>
                    <Heading level={2}>Quarterly plan</Heading>
                    <Text as="p" style={{ lineHeight: 1.75 }}>
                      We keep the scope tight this quarter: ship the layout templates first, then refine the
                      controls. Every screen below is composed from rumahl/ui components.
                    </Text>
                    <Text as="p" tone="muted" style={{ lineHeight: 1.75 }}>
                      The document is plain content on the work surface — no wrapping card, no forced glass.
                    </Text>
                    <RumahlNotice title="Decision" tone="info">
                      Templates are inspiration, not a fixed structure.
                    </RumahlNotice>
                  </article>
                )
              },
              {
                id: "activity",
                label: "Activity",
                content: (
                  <div style={{ ...PANEL, maxWidth: 560 }}>
                    <List>
                      <ListItem title="Alex edited the document" subtitle="2 hours ago" />
                      <ListItem title="Sam shared the project" subtitle="Yesterday" />
                      <ListItem title="Deployed to staging" subtitle="2 days ago" />
                    </List>
                  </div>
                )
              },
              {
                id: "settings",
                label: "Settings",
                content: (
                  <div style={{ ...PANEL, maxWidth: 560 }}>
                    <GroupContainer>
                      <GroupRow title="Comments">
                        <RumahlSwitch label="Comments" defaultChecked />
                      </GroupRow>
                      <GroupRow title="Status" value={<RumahlBadge tone="success">Active</RumahlBadge>} />
                      <GroupRow title="Visibility" value="Private" />
                    </GroupContainer>
                  </div>
                )
              }
            ]}
          />
        </div>
      </div>
    );
  }
};
