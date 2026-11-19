import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, Heading, IconButton, List, ListItem, PageSection, Text } from "@rumahl/ui/components";
import { ArrowLeftIcon, ArrowIcon, DocumentIcon, InfoIcon, SparkIcon } from "@rumahl/ui/icons";

/**
 * # Immersive (media) app layout
 *
 * A template/inspiration for a media detail page (like the artist page): a hero
 * image on top with the controls **floating inside the image**, and the text
 * content below on the work surface. Anything over the image is `floating`.
 */
const meta = {
  title: "Layout/App/Media detail",
  parameters: { layout: "fullscreen" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

const TRACKS = [
  { title: "Golden Hour", time: "3:42" },
  { title: "Nightingale", time: "4:07" },
  { title: "Drift", time: "3:18" },
  { title: "Afterglow", time: "5:02" }
];

export const Default: Story = {
  render: () => (
    <div style={{ height: 560, overflow: "auto" }}>
      {/* Hero image — controls float inside it. */}
      <div
        style={{
          position: "relative",
          height: 260,
          background:
            "radial-gradient(ellipse at 30% 20%, #ffffff22, transparent 55%), linear-gradient(160deg, #103126 0%, #1d4d3a 48%, #123a3a 100%)"
        }}
      >
        <div
          style={{
            position: "absolute",
            inset: 0,
            background: "linear-gradient(180deg, #00000059, transparent 34%, transparent 62%, #00000059)"
          }}
        />

        {/* Controls float as individual islands — no bar around single buttons. */}
        <div style={{ position: "absolute", top: 16, left: 16 }}>
          <IconButton floating shape="circle" label="Back">
            <ArrowLeftIcon />
          </IconButton>
        </div>

        <div style={{ position: "absolute", top: 16, right: 16 }}>
          <IconButton floating shape="circle" label="Info">
            <InfoIcon />
          </IconButton>
        </div>

        <div style={{ position: "absolute", right: 20, bottom: 20 }}>
          <IconButton floating variant="primary" shape="circle" size="xl" label="Play">
            <SparkIcon />
          </IconButton>
        </div>
      </div>

      {/* Text content on the work surface. */}
      <div style={{ padding: "22px 28px 32px", display: "grid", gap: 20 }}>
        <div style={{ display: "grid", gap: 4 }}>
          <Heading level={1} style={{ fontSize: 30}}>
            Summer
          </Heading>
          <Text tone="muted">Aurora · Live set · 12 tracks</Text>
        </div>

        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <Button variant="primary" leftIcon={<SparkIcon />}>
            Play
          </Button>
          <Button leftIcon={<ArrowIcon />}>Shuffle</Button>
          <Button variant="ghost" leftIcon={<DocumentIcon />}>
            Add to library
          </Button>
        </div>

        <PageSection title="Popular">
          <List>
            {TRACKS.map((track) => (
              <ListItem key={track.title} title={track.title} subtitle="Aurora" trailing={<Text tone="muted" size="sm" as="span">{track.time}</Text>} />
            ))}
          </List>
        </PageSection>
      </div>
    </div>
  )
};
