import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Clickable, Text } from "@rumahl/ui/components";

const meta = {
  title: "Primitives/Clickable",
  component: Clickable,
  args: { disabled: false },
  argTypes: { disabled: { control: "boolean" } },
  parameters: {
    docs: {
      description: {
        component: `Wraps arbitrary content into a single accessible target.

## Rules
- **One target, one action**; role and keyboard handling are built in.
- \`disabled\` removes it from the tab order.`
      }
    }
  }
} satisfies Meta<typeof Clickable>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => {
    const [count, setCount] = useState(0);
    return (
      <Clickable
        {...args}
        onActivate={() => setCount((value) => value + 1)}
        style={{ padding: 12, border: "1px solid var(--control-border)", display: "inline-block" }}
      >
        <Text>Activated {count} times — click or press Enter/Space</Text>
      </Clickable>
    );
  }
};
