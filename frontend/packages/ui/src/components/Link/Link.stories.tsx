import type { Meta, StoryObj } from "@storybook/react-vite";
import { Link } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Link",
  component: Link,
  args: { children: "Open documentation", href: "#example" },
  argTypes: {
    external: { control: "boolean" },
    children: { control: "text" },
    href: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `Inline navigation text.

## Rules
- Use for **navigation**; actions use a Button.
- \`external\` adds a glyph and a safe \`target\`/\`rel\`.
- Descriptive text — never "click here".`
      }
    }
  }
} satisfies Meta<typeof Link>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const External: Story = { args: { external: true } };
export const Disabled: Story = { args: { "aria-disabled": true } };
