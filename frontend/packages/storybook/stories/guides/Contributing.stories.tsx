import type { Meta, StoryObj } from "@storybook/react-vite";

/**
 * # Contributing to rumahl/ui
 *
 * ## Component API
 * Keep components predictable and composable:
 * - Native element props are forwarded (`className`, `style`, `aria-*`, …);
 *   `className` is merged, never dropped.
 * - Visual axes: `variant`, `size`, `tone` where they apply.
 * - Cross-cutting element states: `floating` (over a background) and
 *   `glass={false}` (flat). No other one-off booleans.
 * - Controlled and uncontrolled are both supported for inputs.
 * - Type everything; no `any` in public signatures.
 *
 * ## Tokens, never hard-coded values
 * Components read `--rumahl-ui-*` tokens (colour, material, shape, motion,
 * typography). Do not hard-code colours/radii in component CSS: a theme must be
 * able to restyle everything through tokens.
 *
 * ## Accessibility
 * - Prefer native elements (`<button>`, `<input>`, `<fieldset>`).
 * - Every control has an accessible name (label or `aria-label`).
 * - Errors are associated (`aria-describedby`, `aria-invalid`); `role="alert"`
 *   for messages.
 * - Focus-visible styles are required. Verify with the a11y addon.
 *
 * ## Stories
 * One `Name.stories.tsx` per component module, title `Components/Name`, with a
 * lead JSDoc comment on the component (autodocs reads it). Cover: default,
 * variants/sizes, states (hover/focus/disabled/loading), edge cases and the
 * `floating` / `glass={false}` states.
 *
 * ## Tests
 * `Name.test.tsx` next to the component (vitest + @testing-library/react).
 * Assert behaviour and a11y wiring, not snapshots. Run `pnpm --filter
 * @rumahl/ui test`.
 *
 * ## Versioning
 * The public surface is `@rumahl/ui` (tokens, themes, sdk) and
 * `@rumahl/ui/components`. Token ids, component props and event shapes are
 * stable; add, deprecate, then remove — follow semver.
 */
const meta = {
  title: "Guides/Contributing",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: "Standards for components, tokens, stories and tests."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Standards: Story = {
  render: () => (
    <div style={{ maxWidth: 560, fontSize: 13, color: "var(--rumahl-ui-color-text-muted)" }}>
      See the guide above. The rules exist so rumahl/ui stays a predictable,
      themeable design system.
    </div>
  )
};
