import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Chip } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Chip",
  component: Chip,
  args: { children: "Design", selected: false },
  argTypes: { ...nativeArgTypes, selected: { control: "boolean" } },
  parameters: {
    docs: {
      description: {
        component: `A compact, selectable token — filters, tags, quick choices.

- \`selected\` toggles the accent tint; provide \`onClick\` to make it interactive.

## Rules
- Chips are for **filtering / multi-select**, not for actions (use a Button).
- Keep labels to one or two words.
- Signal selection with the accent fill, never colour alone.`
      }
    }
  }
} satisfies Meta<typeof Chip>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Selected: Story = { args: { selected: true } };

export const Removable: Story = {
  render: (args) => <Chip {...args} onRemove={() => {}}>Removable</Chip>
};

export const FilterGroup: Story = {
  render: () => {
    const options = ["Design", "Engineering", "Marketing", "Support"];
    const [active, setActive] = useState<string[]>(["Design"]);
    return (
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        {options.map((option) => (
          <Chip
            key={option}
            selected={active.includes(option)}
            onClick={() =>
              setActive((current) =>
                current.includes(option) ? current.filter((value) => value !== option) : [...current, option]
              )
            }
          >
            {option}
          </Chip>
        ))}
      </div>
    );
  }
};
