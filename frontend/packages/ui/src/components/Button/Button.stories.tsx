import type { CSSProperties } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, ButtonLink } from "@rumahl/ui/components";
import { SettingsIcon, SparkIcon } from "@rumahl/ui/icons";
import { nativeArgTypes } from "../_argtypes";

const BUTTON_DOCS = `The primary action control. A row of actions where **one** action is committed.

- **secondary** — the everyday action (default, glass).
- **primary** — accent-tinted; use **one per view / region**.
- **ghost** — no fill; low emphasis (toolbars, inline).
- **danger** — destructive; never the default action.

## States & props
- \`floating\` — sits over a picture or another surface (its own island). Never
  nest a floating button in a recessed surface.
- \`glass={false}\` — flat fill; \`active\` — pressed/toggled; \`loading\` / \`success\`
  — in-place feedback that keeps the width.
- \`size\` xs–xl, \`leftIcon\`/\`rightIcon\`, \`color\` / \`backgroundColor\`,
  \`leftFlat\`/\`rightFlat\` (join adjacent buttons), \`insetFocus\`.

## Rules
- **Do** keep labels short and verb-first ("Save changes").
- **Do** give icon-only buttons an accessible name (\`aria-label\`).
- **Don't** use \`danger\` for non-destructive actions.
- **Don't** put more than one \`primary\` in a group.
- Native DOM props (\`onClick\`, \`type\`, \`aria-*\`…) are forwarded.`;

const meta = {
  title: "Components/Button",
  component: Button,
  args: { children: "Button", variant: "secondary", size: "md", glass: true },
  parameters: { docs: { description: { component: BUTTON_DOCS } } },
  argTypes: {
    ...nativeArgTypes,
    variant: { control: "inline-radio", options: ["primary", "secondary", "ghost", "danger"] },
    size: { control: "inline-radio", options: ["xs", "sm", "md", "lg", "xl"] },
    floating: { control: "boolean" },
    glass: { control: "boolean" },
    disabled: { control: "boolean" },
    loading: { control: "boolean" },
    success: { control: "boolean" },
    active: { control: "boolean" },
    insetFocus: { control: "boolean" },
    leftFlat: { control: "boolean" },
    rightFlat: { control: "boolean" },
    noSidePadding: { control: "boolean" },
    marginLeft: { control: "boolean" },
    marginRight: { control: "boolean" },
    preventClickAnimation: { control: "boolean" },
    blurOnMouseUp: { control: "boolean" },
    children: { control: "text" },
    successText: { control: "text" },
    color: { control: "color" },
    backgroundColor: { control: "color" }
  }
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Secondary: Story = {};
export const Primary: Story = { args: { variant: "primary" } };
export const Ghost: Story = { args: { variant: "ghost" } };
export const Danger: Story = { args: { variant: "danger" } };
export const Small: Story = { args: { size: "sm" } };
export const Disabled: Story = { args: { disabled: true } };
export const WithTooltip: Story = { args: { title: "Helpful hint" } };
export const Sizes: Story = {
  render: (args) => (
    <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
      {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
        <Button {...args} key={size} size={size}>
          Size {size}
        </Button>
      ))}
    </div>
  )
};
/** Floats over the desktop picture (the preview background image). */
export const Floating: Story = {
  args: { variant: "primary", floating: true },
  parameters: { windowSurface: false },
  render: (args) => <Button {...args} />
};
export const Flat: Story = { args: { glass: false, variant: "primary" } };
const PALETTE = ["#0b84ff", "#22c7a9", "#f45b85", "#ff9f0a", "#8b5cf6"];

export const Colors: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
      {PALETTE.map((color) => (
        <Button key={color} variant="primary" color={color}>
          {color}
        </Button>
      ))}
    </div>
  )
};
export const ColoredVariants: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12 }}>
      {(["secondary", "ghost", "primary", "danger"] as const).map((variant) => (
        <div key={variant} style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <span style={{ width: 72, fontSize: 11, color: "var(--rumahl-ui-color-text-muted)" }}>{variant}</span>
          {PALETTE.map((color) => (
            <Button key={color} variant={variant} color={color}>
              Colored
            </Button>
          ))}
        </div>
      ))}
    </div>
  )
};
export const BackgroundColor: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
      {PALETTE.map((color) => (
        <Button key={color} backgroundColor={color}>
          {color}
        </Button>
      ))}
    </div>
  )
};
export const Loading: Story = { args: { loading: true, variant: "primary" } };
export const Success: Story = { args: { success: true, successText: "Saved", variant: "primary" } };
export const WithIcons: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
      <Button variant="primary" leftIcon={<SparkIcon />}>Create</Button>
      <Button rightIcon={<SettingsIcon />}>Settings</Button>
      <Button variant="ghost" leftIcon={<SparkIcon />} rightIcon={<SettingsIcon />}>Both</Button>
    </div>
  )
};
export const Active: Story = { args: { active: true } };
export const Joined: Story = {
  render: () => (
    <div style={{ display: "flex" }}>
      <Button variant="secondary" rightFlat marginRight>Back</Button>
      <Button variant="secondary" leftFlat>Forward</Button>
    </div>
  )
};
export const AllVariants: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
      <Button variant="secondary">Secondary</Button>
      <Button variant="primary">Primary</Button>
      <Button variant="danger">Danger</Button>
      <Button variant="ghost">Ghost</Button>
    </div>
  )
};
/** Only floating controls belong over the desktop picture. */
export const OverImage: Story = {
  parameters: { windowSurface: false },
  render: () => (
    <div style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
      <Button variant="secondary" floating>Secondary</Button>
      <Button variant="primary" floating>Primary</Button>
      <Button variant="danger" floating>Danger</Button>
      <Button variant="ghost" floating>Ghost</Button>
    </div>
  )
};
export const AsLink: Story = {
  render: () => <ButtonLink href="#example" variant="primary">Open link</ButtonLink>
};

/** Switch the material toolbar to compare the same states without changing content. */
export const ColoredStates: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 20, maxWidth: 560 }}>
      <p style={{ margin: 0, fontSize: 13, color: "var(--rumahl-ui-color-text-muted)" }}>
        Compare Glass, Floating and Solid using the material toolbar. Hover or focus each action to inspect its state.
      </p>
      {(["primary", "danger"] as const).map((variant) => (
        <div key={variant} style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
          <Button variant={variant}>{variant === "primary" ? "Save changes" : "Delete workspace"}</Button>
          <Button variant={variant} size="sm">Small</Button>
          <Button variant={variant} disabled>Disabled</Button>
          <ButtonLink variant={variant} href="#example">Link</ButtonLink>
        </div>
      ))}
    </div>
  )
};

export const GlassSaturation: Story = {
  render: (_args, context) => (
    <div className="shell" data-material="glass" data-scheme={context.globals.scheme === "dark" ? "dark" : "light"} style={{ display: "grid", gap: 20, maxWidth: 600 }}>
      <p style={{ margin: 0, fontSize: 13 }}>Switch Light / Dark in the toolbar to compare the original colour with the tinted glass surface.</p>
      {["#00c98d", "#0088ff", "#e832ba", "#f27b16"].map(color => (
        <div key={color} style={{ display: "flex", gap: 14, alignItems: "center", flexWrap: "wrap", "--rumahl-ui-color-accent": color } as CSSProperties}>
          <span aria-label={`Original pigment ${color}`} style={{ width: 28, height: 28, borderRadius: 6, background: color }} />
          <Button>Empty glass</Button>
          <Button variant="primary" style={{ "--glass-inner-glow": "0%", "--glass-outer-glow": "0%" } as CSSProperties}>Without glow</Button>
          <Button variant="primary">Soft glow</Button>
          <Button variant="primary" disabled>Disabled</Button>
        </div>
      ))}
    </div>
  )
};
