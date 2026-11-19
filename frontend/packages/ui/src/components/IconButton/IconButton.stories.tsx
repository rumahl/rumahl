import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "@rumahl/ui/components";
import { SettingsIcon } from "@rumahl/ui/icons";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Primitives/IconButton",
  component: IconButton,
  args: { label: "Settings", size: "md", shape: "square", variant: "secondary", children: <SettingsIcon /> },
  argTypes: {
    ...nativeArgTypes,
    label: { control: "text" },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    shape: { control: "inline-radio", options: ["square", "circle"] },
    variant: { control: "inline-radio", options: ["secondary", "primary", "ghost"] },
    disabled: { control: "boolean" },
    loading: { control: "boolean" },
    active: { control: "boolean" },
    title: { control: "text" },
    children: { control: false },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  },
  parameters: {
    docs: {
      description: {
        component: `A labelled, icon-only action.

## Rules
- **Always provide \`label\`** (the accessible name); \`title\` is the tooltip.
- One icon, one meaning. \`shape="circle"\` for floating / status; \`square\` otherwise.
- \`variant\` secondary / primary / ghost; \`floating\` over a picture.`
      }
    }
  }
} satisfies Meta<typeof IconButton>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Circle: Story = { args: { shape: "circle" } };
export const Primary: Story = { args: { variant: "primary" } };
export const Sizes: Story = {
  render: (args) => (
    <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <IconButton {...args} key={size} size={size} />
      ))}
    </div>
  )
};
export const Loading: Story = { args: { loading: true } };
export const Active: Story = { args: { active: true } };
export const WithTooltip: Story = { args: { title: "Open settings" } };
export const AsLink: Story = {
  render: (args) => (
    <IconButton {...args} as="a" href="#settings" label="Settings" />
  )
};
