import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, RumahlNotice } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Notice",
  component: RumahlNotice,
  args: { title: "Information", children: "A new version is available.", tone: "info" },
  argTypes: { ...nativeArgTypes, tone: { control: "inline-radio", options: ["info", "warning", "danger"] },
    color: { control: "color" },
    backgroundColor: { control: "color" } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `An inline, full-width message: info, warning or danger, with an optional action.

## Rules
- Inline for **contextual** messages; a Toast for transient events.
- Lead with the consequence; keep the action beside the text.
- Don't use \`danger\` for non-critical information.`
      }
    }
  }
} satisfies Meta<typeof RumahlNotice>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithAction: Story = {
  args: {
    tone: "danger",
    title: "Action needed",
    action: <Button size="sm">Fix now</Button>
  }
};

export const Tones: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12, width: 460 }}>
      <RumahlNotice title="Information">A new version is available.</RumahlNotice>
      <RumahlNotice title="Warning" tone="warning">Storage is running low.</RumahlNotice>
      <RumahlNotice title="Action needed" tone="danger" action={<Button size="sm">Fix now</Button>}>
        The backup could not be completed.
      </RumahlNotice>
    </div>
  )
};

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12, width: 460 }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <RumahlNotice key={size} size={size} title={`Size ${size}`}>
          Callout at size {size}.
        </RumahlNotice>
      ))}
    </div>
  )
};
