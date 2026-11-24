import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button as SubpathButton } from "@rumahl/ui/components/Button";
import { Button, RumahlInput } from "@rumahl/ui/components";

/**
 * # Architecture
 *
 * `@rumahl/ui` is self-contained (no dependency on the shell), so it can be
 * extracted into its own repository together with this Storybook.
 *
 * ## Package layout
 * - `color.ts` · `palette.ts` · `tokens.ts` — foundations (data)
 * - `themes.ts` · `variants.ts` · `sdk.ts` — theme model + authoring SDK
 * - `ThemeProvider.tsx` — projects tokens onto `:root`
 * - `materials.css` · `default-theme.css` — material + theme layers
 * - `components/<Name>/` — one folder per component (`<Name>.tsx`,
 *   `<Name>.css`, `<Name>.test.tsx`, `<Name>.stories.tsx`)
 * - `icons.tsx` · `i18n/` · `slots.tsx`
 *
 * ## How to import
 *
 * ```ts
 * // Everything (barrel):
 * import { Button, RumahlInput } from "@rumahl/ui/components";
 *
 * // A single component (tree-shakeable subpath):
 * import { Button } from "@rumahl/ui/components/Button";
 *
 * // Theme + providers + foundations:
 * import { ThemeProvider, useTheme, defaultTheme } from "@rumahl/ui";
 * import { defineTheme, registerTheme } from "@rumahl/ui/sdk";
 * ```
 *
 * ## Styles
 * Import once, before your own CSS: `@rumahl/ui/materials.css`,
 * `@rumahl/ui/default-theme.css`, `@rumahl/ui/components/controls.css`,
 * `@rumahl/ui/components/scrollbars.css`.
 *
 * ## Extraction
 * Moving the package + Storybook to a new repo needs only the workspace deps
 * (`react`, `react-dom`, `@rumahl/contracts` types) and `pnpm install`.
 */
const meta = {
  title: "Guides/Architecture",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: "Package structure, public exports and extraction."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Imports: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 14, width: 360 }}>
      <Button variant="primary">From the barrel</Button>
      <SubpathButton>From the Button subpath</SubpathButton>
      <RumahlInput placeholder="A field from the barrel" />
    </div>
  )
};
