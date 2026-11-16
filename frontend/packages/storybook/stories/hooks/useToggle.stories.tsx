import type { Meta, StoryObj } from "@storybook/react-vite";
import { useToggle } from "@rumahl/ui/hooks";
import { Button, RumahlSwitch, Text } from "@rumahl/ui/components";

const meta = { title: "Hooks/useToggle", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [on, toggle] = useToggle();
    return (
      <div style={{ display: "grid", gap: 12, justifyItems: "start" }}>
        <RumahlSwitch label={`Value: ${on}`} checked={on} onChange={() => toggle()} />
        <Button onClick={() => toggle()}>Toggle</Button>
        <Button onClick={() => toggle(true)}>Force on</Button>
        <Text tone="muted">Stable toggler: `toggle()`, `toggle(true)`, `toggle(false)`.</Text>
      </div>
    );
  }
};
