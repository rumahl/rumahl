import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";
import { Button, RumahlInput, RumahlSwitch } from "@rumahl/ui/components";

/**
 * # Getting started
 *
 * Build your own app with the rumahl/ui building blocks and themes.
 *
 * ## Install
 *
 * ```sh
 * pnpm add @rumahl/ui
 * ```
 *
 * In this monorepo it is a workspace package: `"@rumahl/ui": "workspace:*"`.
 *
 * ## Styles
 *
 * Import the material layer and the control styles once, before your own CSS:
 *
 * ```ts
 * import "@rumahl/ui/materials.css";        // surface + material tokens
 * import "@rumahl/ui/default-theme.css";    // the rumahl default theme
 * import "@rumahl/ui/components/controls.css";
 * import "@rumahl/ui/components/scrollbars.css";
 * ```
 *
 * ## Providers
 *
 * Wrap your app in `ThemeProvider` (projects tokens onto `:root`) and, for
 * translated components, `I18nProvider`.
 *
 * ```tsx
 * import { ThemeProvider, defaultTheme } from "@rumahl/ui";
 * import { I18nProvider } from "@rumahl/ui/i18n";
 *
 * <ThemeProvider theme={defaultTheme}>
 *   <I18nProvider locale="en">
 *     <App />
 *   </I18nProvider>
 * </ThemeProvider>
 * ```
 *
 * ## Components
 *
 * ```tsx
 * import { Button, RumahlInput, RumahlSwitch } from "@rumahl/ui/components";
 *
 * <Button variant="primary">Save</Button>
 * <RumahlInput placeholder="Name" />
 * ```
 *
 * Every component accepts native props (`className`, `style`, `aria-*`) and the
 * override props `glass={false}` and `floating` — see **Guides/Overrides**.
 */
const meta = {
  title: "Guides/Getting started",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: "Installation, styles, providers and first components."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Hello() {
  const { theme } = useTheme();
  return (
    <div style={{ display: "grid", gap: 14, width: 360 }}>
      <strong>Hello from {theme.name}</strong>
      <RumahlInput placeholder="Name" />
      <RumahlSwitch label="Enable" defaultChecked />
      <Button variant="primary">Save</Button>
    </div>
  );
}

export const FirstComponents: Story = {
  render: () => <Hello />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { Button, RumahlInput, RumahlSwitch } from "@rumahl/ui/components";

export function App() {
  return (
    <div style={{ display: "grid", gap: 14, width: 360 }}>
      <RumahlInput placeholder="Name" />
      <RumahlSwitch label="Enable" defaultChecked />
      <Button variant="primary">Save</Button>
    </div>
  );
}`
      }
    }
  }
};
