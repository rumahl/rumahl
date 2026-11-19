import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  GroupContainer,
  GroupRow,
  Heading,
  IconButton,
  List,
  ListItem,
  PhoneFrame,
  RumahlAvatar,
  RumahlBadge
} from "@rumahl/ui/components";
import { ArrowLeftIcon, ArrowIcon } from "@rumahl/ui/icons";

/**
 * # Mobile list–detail (hybrid)
 *
 * A template/inspiration for a push navigation: a list screen that opens a
 * detail screen inside the same `PhoneFrame` (with a back button).
 */
const meta = { title: "Layout/Mobile/List–detail", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const PEOPLE = [
  { id: "alex", name: "Alex Morgan", detail: "Owner · alex@rumahl.dev" },
  { id: "sam", name: "Sam Rivera", detail: "Editor · sam@rumahl.dev" },
  { id: "jo", name: "Jo Chen", detail: "Viewer · jo@rumahl.dev" }
];

const status = (
  <>
    <span>9:41</span>
    <span style={{ letterSpacing: 1, opacity: 0.7 }}>••• ▮</span>
  </>
);

export const Default: Story = {
  render: () => {
    const [openId, setOpenId] = useState<string | null>(null);
    const person = PEOPLE.find((p) => p.id === openId) ?? null;
    return (
      <div style={{ minHeight: 700, display: "grid", placeItems: "center", padding: 24, background: "var(--rumahl-ui-color-canvas-background)" }}>
        <PhoneFrame status={status}>
          {person ? (
            <div style={{ padding: "0 18px 24px", display: "grid", gap: 14 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "6px 0" }}>
                <IconButton shape="circle" variant="ghost" label="Back" onClick={() => setOpenId(null)}>
                  <ArrowLeftIcon />
                </IconButton>
                <Heading level={4}>{person.name}</Heading>
              </div>
              <GroupContainer>
                <GroupRow title="Role" value={person.detail.split(" · ")[0]} />
                <GroupRow title="Email" value={person.detail.split(" · ")[1]} />
                <GroupRow title="Status" value={<RumahlBadge tone="success">Active</RumahlBadge>} />
              </GroupContainer>
            </div>
          ) : (
            <div style={{ padding: "8px 18px" }}>
              <Heading level={3} style={{ marginBottom: 12 }}>
                People
              </Heading>
              <List>
                {PEOPLE.map((p) => (
                  <ListItem
                    key={p.id}
                    interactive
                    onClick={() => setOpenId(p.id)}
                    leading={<RumahlAvatar name={p.name} size="sm" />}
                    title={p.name}
                    subtitle={p.detail}
                    trailing={
                      <span style={{ display: "inline-flex", color: "var(--rumahl-ui-color-text-muted)" }}>
                        <ArrowIcon />
                      </span>
                    }
                  />
                ))}
              </List>
            </div>
          )}
        </PhoneFrame>
      </div>
    );
  }
};
