import type { Meta, StoryObj } from "@storybook/react-vite";
import { TextWithHighlight } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/TextWithHighlight",
  component: TextWithHighlight,
  args: { text: "Search results for the calendar app", highlight: "calendar", size: "md" },
  argTypes: {
    text: { control: "text" },
    highlight: { control: "text" },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] }
  },
  parameters: {
    docs: {
      description: {
        component: `Text with a matched substring emphasised — for search results.

## Rules
- Only to show **why** a result matched; never style unrelated words.
- Keeps the surrounding text intact.`
      }
    }
  }
} satisfies Meta<typeof TextWithHighlight>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
