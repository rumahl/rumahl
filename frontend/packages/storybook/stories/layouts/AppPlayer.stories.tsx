import type { Meta, StoryObj } from "@storybook/react-vite";
import { Flex, Heading, IconButton, List, ListItem, RumahlProgress, Text } from "@rumahl/ui/components";
import { ArrowIcon, ArrowLeftIcon, SparkIcon } from "@rumahl/ui/icons";

/**
 * # Player / now-playing (app style)
 *
 * A template/inspiration: large artwork + transport controls, with the queue
 * alongside. Content on the surface.
 */
const meta = { title: "Layout/App/Player", parameters: { layout: "fullscreen" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const QUEUE = [
  { title: "Golden Hour", time: "3:42" },
  { title: "Nightingale", time: "4:07" },
  { title: "Drift", time: "3:18" },
  { title: "Afterglow", time: "5:02" }
];

export const Default: Story = {
  render: () => (
    <div style={{ height: 560, display: "grid", gridTemplateColumns: "minmax(0, 1fr) 320px" }}>
      <div style={{ display: "grid", placeItems: "center", padding: 24 }}>
        <div style={{ display: "grid", gap: 20, justifyItems: "center", width: "min(420px, 90%)" }}>
          <div
            style={{
              width: 240,
              height: 240,
              borderRadius: 20,
              background: "radial-gradient(ellipse at 30% 20%, #ffffff33, transparent 55%), linear-gradient(150deg, #1f6f5c, #2b6cb0 60%, #b83280)"
            }}
          />
          <div style={{ textAlign: "center", display: "grid", gap: 2 }}>
            <Heading level={2}>Golden Hour</Heading>
            <Text tone="muted">Aurora · Summer</Text>
          </div>
          <div style={{ width: "100%" }}>
            <RumahlProgress label="Track" value={38} />
          </div>
          <Flex align="center" gap={3}>
            <IconButton shape="circle" label="Previous">
              <ArrowLeftIcon />
            </IconButton>
            <IconButton shape="circle" variant="primary" size="lg" label="Play">
              <SparkIcon />
            </IconButton>
            <IconButton shape="circle" label="Next">
              <ArrowIcon />
            </IconButton>
          </Flex>
        </div>
      </div>

      <div style={{ borderLeft: "1px solid var(--surface-edge)", padding: 16, overflow: "auto" }}>
        <Heading level={6} style={{ marginBottom: 8 }}>
          Up next
        </Heading>
        <List>
          {QUEUE.map((track) => (
            <ListItem key={track.title} title={track.title} subtitle="Aurora" trailing={<Text tone="muted" size="sm" as="span">{track.time}</Text>} />
          ))}
        </List>
      </div>
    </div>
  )
};
