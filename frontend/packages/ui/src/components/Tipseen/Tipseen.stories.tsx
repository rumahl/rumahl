import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, Tipseen } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Tipseen",
  component: Tipseen,
  argTypes: { ...nativeArgTypes, color: { control: "color" }, backgroundColor: { control: "color" } },
  args: {
    title: "Welcome to the launcher",
    children: "Press the launcher icon to open your apps. You can rearrange them any time."
  },
  parameters: {
    docs: {
      description: {
        component: `A coach-mark popover for onboarding.

## Rules
- **One tip, one idea**; always dismissible and non-blocking.
- Never rely on a tip for required information.`
      }
    }
  }
} satisfies Meta<typeof Tipseen>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => <Tipseen {...args} onClose={() => {}} />
};

export const WithActions: Story = {
  render: (args) => (
    <Tipseen
      {...args}
      onClose={() => {}}
      actions={
        <>
          <Button size="sm">Got it</Button>
          <Button size="sm" variant="ghost">Learn more</Button>
        </>
      }
    />
  )
};
