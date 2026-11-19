import type { Meta, StoryObj } from "@storybook/react-vite";
import { AvatarGroup, RumahlAvatar } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const NAMES = ["Alex Morgan", "Sam Rivera", "Jo Chen", "Pat Kim", "Dana Roy"];

const meta = {
  title: "Components/AvatarGroup",
  component: AvatarGroup,
  args: { max: 3, children: null },
  argTypes: { ...nativeArgTypes, max: { control: { type: "number" } } },
  parameters: {
    docs: {
      description: {
        component: `Overlapping avatars with a \`+N\` overflow.

## Rules
- \`max\` caps the visible avatars; the rest are counted.
- Order by relevance (owner first).
- Pair with a label or tooltip so the members are identifiable.`
      }
    }
  }
} satisfies Meta<typeof AvatarGroup>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <AvatarGroup {...args}>
      {NAMES.map((name) => (
        <RumahlAvatar key={name} name={name} size="md" />
      ))}
    </AvatarGroup>
  )
};

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16 }}>
      {(["sm", "md", "lg"] as const).map((size) => (
        <AvatarGroup key={size} max={3}>
          {NAMES.map((name) => (
            <RumahlAvatar key={name} name={name} size={size} />
          ))}
        </AvatarGroup>
      ))}
    </div>
  )
};
