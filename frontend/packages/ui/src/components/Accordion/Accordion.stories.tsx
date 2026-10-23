import type { Meta, StoryObj } from "@storybook/react-vite";
import { Accordion } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const ITEMS = [
  { id: "general", title: "General", content: "General preferences live here. Change them any time." },
  { id: "network", title: "Network", content: "Network and connectivity options." },
  { id: "about", title: "About", content: "Version and legal information." }
];

const meta = {
  title: "Components/Accordion",
  component: Accordion,
  args: { items: ITEMS, defaultOpen: ["general"] },
  argTypes: { ...nativeArgTypes, multiple: { control: "boolean" }, color: { control: "color" } },
  parameters: {
    docs: {
      description: {
        component: `Progressive disclosure: a stack of collapsible sections.

- \`multiple\` — allow several sections open at once.
- \`defaultOpen\` — ids open on first render; \`color\` tints the active header.

## Rules
- One idea per section; the header is the label, the body the detail.
- Use it for secondary detail — **not** for primary navigation.
- Don't bury required content in a collapsed section without a cue.`
      }
    }
  }
} satisfies Meta<typeof Accordion>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Single: Story = {};
export const Multiple: Story = { args: { multiple: true, defaultOpen: ["general", "network"] } };
