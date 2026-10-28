import type { Meta, StoryObj } from "@storybook/react-vite";
import { HiddenText } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/HiddenText",
  component: HiddenText,
  args: { children: "Opens in a new tab" },
  argTypes: { children: { control: "text" } },
  parameters: {
    docs: {
      description: {
        component: `Visually hidden text for screen readers.

## Rules
- Use to name icon-only controls or add context.
- Keep it concise — it is announced, not seen.`
      }
    }
  }
} satisfies Meta<typeof HiddenText>;
export default meta;
type Story = StoryObj<typeof meta>;

/** The hidden text is not visible; it is announced by screen readers. */
export const Default: Story = {
  render: (args) => (
    <p style={{ fontSize: 14 }}>
      Read more <HiddenText>{args.children}</HiddenText>
    </p>
  )
};
