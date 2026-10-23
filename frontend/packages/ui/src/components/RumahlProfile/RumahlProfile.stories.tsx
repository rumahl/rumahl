import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  RumahlAvatar,
  RumahlAvatarChange,
  RumahlPassword,
  RumahlProfileCard,
  RumahlProfileSwitch
} from "@rumahl/ui/components";
import type { ControlSize } from "@rumahl/ui";

const PROFILES = [
  { id: "alex", name: "Alex Morgan", detail: "alex@rumahl.dev" },
  { id: "sam", name: "Sam Rivera", detail: "sam@rumahl.dev" }
];

const SIZE_OPTIONS = ["xs", "sm", "md", "lg", "xl"] as const;

const meta = {
  title: "Components/Profile",
  args: { size: "md" as ControlSize },
  argTypes: {
    size: { control: "inline-radio", options: SIZE_OPTIONS }
  },
  parameters: {
    docs: {
      description: {
        component: `The signed-in identity (avatar + name + meta).

## Rules
- Shows the **active** account; size follows its container.
- The account switcher lives in menus, not inline.`
      }
    }
  }
} satisfies Meta<{ size?: ControlSize }>;
export default meta;
type Story = StoryObj<{ size?: ControlSize }>;

export const Avatar: Story = {
  render: (args) => (
    <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
      <RumahlAvatar name="Alex Morgan" size={args.size} />
      <RumahlAvatar name="Alex Morgan" size={args.size} status="online" />
      <RumahlAvatar name="Sam Rivera" size={args.size} status="away" />
    </div>
  )
};

export const AvatarSizes: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
      {SIZE_OPTIONS.map((size) => (
        <RumahlAvatar key={size} name="Alex Morgan" size={size} />
      ))}
    </div>
  )
};

export const ProfileCard: Story = {
  render: () => <RumahlProfileCard profile={PROFILES[0]!} status="Signed in" />
};

export const ProfileSwitch: Story = {
  render: () => {
    const [value, setValue] = useState("alex");
    return <RumahlProfileSwitch profiles={PROFILES} value={value} onChange={setValue} />;
  }
};

export const Password: Story = {
  render: () => <RumahlPassword label="Password" defaultValue="hunter2" />
};

export const AvatarChange: Story = {
  render: () => <RumahlAvatarChange name="Alex Morgan" onChange={() => {}} />
};
