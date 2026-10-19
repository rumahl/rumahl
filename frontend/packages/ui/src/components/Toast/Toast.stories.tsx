import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, Toast, ToastStack, type ToastData } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const meta = {
  title: "Components/Toast",
  component: Toast,
  args: { title: "Update available", message: "A new version is ready to install.", tone: "info" },
  argTypes: {
    ...nativeArgTypes,
    tone: { control: "inline-radio", options: ["info", "success", "warning", "danger"] },
    title: { control: "text" },
    message: { control: "text" }
  },
  parameters: {
    windowSurface: false,
    docs: {
      description: {
        component: `A transient, non-blocking notification that stacks in a corner.

## Rules
- Toasts are for **transient** events; persistent messages are Notices.
- One line of consequence plus an optional action; they auto-dismiss.
- Never gate a required action on a toast.`
      }
    }
  }
} satisfies Meta<typeof Toast>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = { render: (args) => <Toast {...args} onClose={() => {}} /> };

export const Tones: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 10 }}>
      {(["info", "success", "warning", "danger"] as const).map((tone) => (
        <Toast key={tone} tone={tone} title={tone} message={`A ${tone} notification.`} />
      ))}
    </div>
  )
};

export const Stack: Story = {
  render: () => {
    const [toasts, setToasts] = useState<ToastData[]>([]);
    return (
      <>
        <Button
          onClick={() =>
            setToasts((current) => [
              ...current,
              { id: String(Date.now()), title: "Saved", message: "Your changes were saved.", tone: "success" }
            ])
          }
        >
          Show toast
        </Button>
        <ToastStack toasts={toasts} onClose={(id) => setToasts((current) => current.filter((toast) => toast.id !== id))} />
      </>
    );
  }
};
