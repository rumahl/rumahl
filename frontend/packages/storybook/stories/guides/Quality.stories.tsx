import type { Meta, StoryObj } from "@storybook/react-vite";

/**
 * # Quality & tooling
 *
 * The pipeline that keeps rumahl/ui trustworthy as a public design system.
 *
 * ## Accessibility
 * The a11y addon is installed and violations are treated as errors in autodocs
 * and tests (`a11y.test = "error"`, set globally in `preview`). Run the stories
 * through an automated a11y pass:
 *
 * ```sh
 * pnpm add -D @storybook/test-runner
 * pnpm exec test-storybook            # against a running Storybook
 * ```
 *
 * ## Visual regression
 * Publish Storybook publicly and let a hosted service diff every story:
 *
 * ```sh
 * pnpm dlx chromatic --project-token=<token>
 * ```
 *
 * ## Bundle budget
 * Guard the package size (tokens/themes must stay tiny):
 *
 * ```jsonc
 * "size-limit": [
 *   { "path": "src/index.ts", "limit": "10 kB" },
 *   { "path": "src/components/index.ts", "limit": "40 kB" }
 * ]
 * ```
 *
 * ## Versioning / API stability
 * The public surface is `@rumahl/ui` (tokens, themes, sdk, materials) and
 * `@rumahl/ui/components`. Token ids, component props and event shapes are
 * stable and follow semver: add → deprecate → remove. A change to a token id or
 * a prop is a breaking change.
 *
 * ## Tests
 * Behaviour tests live next to each component (`Name.test.tsx`, vitest +
 * @testing-library/react). Run `pnpm --filter @rumahl/ui test`; type-check with
 * `pnpm --filter @rumahl/ui typecheck`.
 */
const meta = {
  title: "Guides/Quality",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component:
          "Accessibility, visual regression, bundle budget and API stability — " +
          "how to keep the design system trustworthy."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Pipeline: Story = {
  render: () => (
    <div style={{ maxWidth: 560, fontSize: 13, color: "var(--rumahl-ui-color-text-muted)" }}>
      a11y ✔ enforceable · visual regression &amp; size budget configurable ·
      semver-stable public API.
    </div>
  )
};
