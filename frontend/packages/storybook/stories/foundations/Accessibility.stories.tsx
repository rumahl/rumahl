import type { Meta, StoryObj } from "@storybook/react-vite";
import { Clickable, HiddenText, Text } from "@rumahl/ui/components";

/**
 * # Accessibility
 *
 * The kit follows a few rules so apps inherit good behaviour:
 *
 * - **Focus**: every interactive control shows a visible focus ring
 *   (`outline: 2px solid var(--rumahl-ui-color-accent)`); never remove it.
 * - **Contrast**: text uses `--rumahl-ui-color-text-primary`/`-muted` against
 *   the surface; accent/on-accent pairs are checked by the colour system.
 * - **Reduced motion**: animations are disabled under
 *   `prefers-reduced-motion: reduce` and `[data-animations="off"]`.
 * - **Forced colors**: switches fall back to native rendering.
 * - **Names**: icon-only controls take a `label`; extra context for screen
 *   readers uses `HiddenText`; composite targets use `Clickable`.
 *
 * The a11y addon runs on every story and fails on violations.
 */
const meta = {
  title: "Foundations/Accessibility",
  tags: ["autodocs"],
  parameters: { layout: "centered" }
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const FocusAndHiddenText: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 16, maxWidth: 360 }}>
      <Text>
        Press <kbd>Tab</kbd> to move focus — the ring is always visible.
      </Text>
      <Clickable style={{ padding: 12, border: "1px solid var(--control-border)" }}>
        <Text>A composite target, keyboard-activatable</Text>
      </Clickable>
      <Text>
        Read more <HiddenText>about accessibility</HiddenText>
      </Text>
    </div>
  )
};
