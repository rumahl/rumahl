import type { Meta, StoryObj } from "@storybook/react-vite";
import { Skeleton } from "@rumahl/ui/components";

const meta = {
  title: "Components/Skeleton",
  component: Skeleton,
  args: { variant: "rect", width: 200, height: 80 },
  argTypes: {
    variant: { control: "inline-radio", options: ["rect", "text", "circle"] },
    width: { control: "text" },
    height: { control: "text" },
    lines: { control: { type: "number" } }
  },
  parameters: {
    docs: {
      description: {
        component: `Loading placeholders that mirror the shape of the content to come
(rect / circle / text, or \`lines\` for a paragraph).

## Rules
- **Match the real layout** so the page doesn't jump when content arrives.
- Skeletons for known layouts; a Spinner for unknown waits.
- The shimmer honours reduced motion.`
      }
    }
  }
} satisfies Meta<typeof Skeleton>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Circle: Story = { args: { variant: "circle", width: 56, height: 56 } };
export const Paragraph: Story = { args: { variant: "text", lines: 4 } };
export const Card: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 12, width: 300 }}>
      <Skeleton variant="circle" width={48} height={48} />
      <div style={{ flex: 1 }}>
        <Skeleton variant="text" width="60%" />
        <div style={{ height: 8 }} />
        <Skeleton variant="text" lines={3} />
      </div>
    </div>
  )
};
