import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Button,
  GroupContainer,
  GroupRow,
  PageHeader,
  PageSection,
  RumahlEmptyState,
  RumahlSwitch
} from "@rumahl/ui/components";

const LAYOUT_DOCS = `# Layout

Page structure: **one heading** (\`PageHeader\`), grouped sections
(\`PageSection\` + \`GroupContainer\`/\`GroupRow\`) and an empty state
(\`RumahlEmptyState\`) when there is nothing to show.

## Do
- **One \`PageHeader\` per page**, with the page's actions beside it.
- Section the page with \`PageSection\` titles and grouped rows — not with boxes.
- **Give every list an empty state**: explain what goes here and offer the action.

## Don't
- Don't put material on the content — material is for chrome (sidebar, frame).
- Don't nest cards in cards; the group surface is the only box.
- Don't invent section boxes — use the \`PageSection\` title + a group.`;

const meta = {
  title: "Patterns/Layout",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { description: { component: LAYOUT_DOCS } }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Page: Story = {
  render: () => (
    <div style={{ padding: 28, maxWidth: 720 }}>
      <PageHeader title="Network" description="Connections, addresses and sharing.">
        <PageSection title="Interfaces">
          <GroupContainer>
            <GroupRow title="Automatic (DHCP)">
              <RumahlSwitch label="Automatic (DHCP)" defaultChecked />
            </GroupRow>
          </GroupContainer>
        </PageSection>
        <PageSection title="Sharing">
          <GroupContainer>
            <GroupRow title="Share this device's files">
              <RumahlSwitch label="Share this device's files" />
            </GroupRow>
          </GroupContainer>
        </PageSection>
      </PageHeader>
    </div>
  )
};

/**
 * When a page has no content yet, the *page body* is the empty state — keep the
 * heading and put the explanation + action where the content would be.
 */
export const Empty: Story = {
  render: () => (
    <div style={{ padding: 28, maxWidth: 720 }}>
      <PageHeader title="Apps" description="Applications installed on this device.">
        <RumahlEmptyState
          eyebrow="No apps yet"
          title="Nothing installed"
          action={
            <div style={{ display: "flex", gap: 10 }}>
              <Button variant="primary">Open the app store</Button>
              <Button>Install from file…</Button>
            </div>
          }
        >
          Apps you install will appear here, alongside the built-in ones.
        </RumahlEmptyState>
      </PageHeader>
    </div>
  )
};
