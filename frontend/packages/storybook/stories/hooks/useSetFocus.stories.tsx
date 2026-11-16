import type { Meta, StoryObj } from "@storybook/react-vite";
import { useSetFocus } from "@rumahl/ui/hooks";
import { Button, RumahlInput } from "@rumahl/ui/components";

const meta = { title: "Hooks/useSetFocus", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [ref, focus] = useSetFocus<HTMLInputElement>();
    return (
      <div style={{ display: "grid", gap: 10, justifyItems: "start" }}>
        <RumahlInput ref={ref} placeholder="Focus me" />
        <Button onClick={focus}>Focus the field</Button>
      </div>
    );
  }
};
