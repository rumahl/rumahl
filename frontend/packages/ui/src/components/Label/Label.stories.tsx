import type { Meta, StoryObj } from "@storybook/react-vite";
import { Label } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Label",
  component: Label,
  args: { children: "Display name", tone: "default" },
  argTypes: {
    required: { control: "boolean" },
    optionalText: { control: "text" },
    tone: { control: "inline-radio", options: ["default", "muted"] },
    children: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `A field label with required / optional affordances.

## Rules
- **Always label inputs**; mark \`required\` or show \`optionalText\`.
- A placeholder is **not** a label.`
      }
    }
  }
} satisfies Meta<typeof Label>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Required: Story = { args: { required: true } };
export const Optional: Story = { args: { optionalText: "Optional" } };
