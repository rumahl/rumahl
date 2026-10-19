import type { Meta, StoryObj } from "@storybook/react-vite";
import { Heading } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Heading",
  component: Heading,
  args: { children: "Heading", level: 2, tone: "default" },
  argTypes: {
    level: { control: "inline-radio", options: [1, 2, 3, 4, 5, 6] },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    tone: { control: "inline-radio", options: ["default", "muted"] },
    children: { control: "text" }
  },
  parameters: {
    docs: {
      description: {
        component: `A semantic heading (\`level\` 1–6) with an independent visual \`size\`.

## Rules
- \`level\` sets the **semantics**, \`size\` the visuals — keep the outline logical.
- **One \`level={1}\` per page.**`
      }
    }
  }
} satisfies Meta<typeof Heading>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Levels: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 10 }}>
      {([1, 2, 3, 4, 5, 6] as const).map((level) => (
        <Heading key={level} level={level}>
          Level {level}
        </Heading>
      ))}
    </div>
  )
};
