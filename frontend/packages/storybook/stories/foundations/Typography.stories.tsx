import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";

/**
 * # Typography
 *
 * Typography roles. `typography.family` is the UI face and `typography.display`
 * the statement face; both are font-stack tokens a theme can override. The
 * approved Iris role map is 12px chrome/labels, 12.5px compact, 13px body,
 * 14px controls, 15px window titles and 30px page titles. The UI uses tight
 * tracking (−0.01…−0.03em) as sizes grow.
 *
 * ## Rules
 *
 * - **Use the text roles** (`Text`, `Heading`) and `--type-*` roles — avoid
 *   ad-hoc `font-size`.
 * - **12px is the floor.** A `<small>` default may not shrink functional text.
 *   Global `space.*` tokens are layout-level; optical component spacing stays
 *   in the component recipe.
 * - **`display` is for statement headings only**; body and controls use
 *   `family`.
 * - **Muted, not lighter.** Secondary text uses
 *   `--rumahl-ui-color-text-muted`, not a reduced-opacity primary.
 * - **Keep line length in check** (~65–75ch) for reading content.
 */
const meta = {
  title: "Foundations/Typography",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Specimen() {
  const { tokens } = useTheme();
  return (
    <div style={{ display: "grid", gap: 28, padding: 24, maxWidth: 720 }}>
      <div>
        <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>typography.display</div>
        <div style={{ fontFamily: tokens["typography.display"], fontSize: 48, fontWeight: 500, letterSpacing: "-.02em" }}>
          Good evening
        </div>
      </div>
      <div>
        <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>typography.family</div>
        <div style={{ fontFamily: tokens["typography.family"] }}>
          <p style={{ fontSize: 18, margin: "0 0 6px" }}>The quick brown fox jumps over the lazy dog.</p>
          <p style={{ fontSize: 14, margin: "0 0 6px" }}>The quick brown fox jumps over the lazy dog.</p>
          <p style={{ fontSize: 12, margin: 0 }}>The quick brown fox jumps over the lazy dog.</p>
        </div>
      </div>
      <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "6px 16px", fontSize: 12}}>
        {["typography.family", "typography.display", "typography.scale"].map((key) => (
          <div key={key} style={{ display: "contents" }}>
            <dt style={{ color: "var(--rumahl-ui-color-text-muted)" }}>{key}</dt>
            <dd style={{ margin: 0, overflowWrap: "anywhere" }}>{(tokens as Record<string, string>)[key]}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export const Specimens: Story = {
  render: () => <Specimen />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { Heading, Text } from "@rumahl/ui/components";

<Heading level={1}>Page title</Heading>
<Text>Body copy uses the UI face.</Text>
<Text tone="muted">Secondary text uses the muted role.</Text>`
      }
    }
  }
};

const SCALE = [
  { name: "title", px: 30 },
  { name: "xl", px: 24 },
  { name: "lg", px: 20 },
  { name: "md", px: 16 },
  { name: "window", px: 15 },
  { name: "control", px: 14 },
  { name: "body", px: 13 },
  { name: "compact", px: 12.5 },
  { name: "label", px: 12 }
];

function Scale() {
  const { tokens } = useTheme();
  const t = tokens as Record<string, string>;
  const family = t["typography.family"] ?? "system-ui, sans-serif";
  return (
    <div style={{ display: "grid", gap: 16, padding: 24, maxWidth: 760 }}>
      <div style={{ display: "flex", gap: 12, fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>
        <span style={{ width: 48 }}>token</span>
        <span style={{ width: 40 }}>px</span>
        <span>sample</span>
      </div>
      {SCALE.map((step) => (
        <div key={step.name} style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
          <code style={{ width: 48, fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{step.name}</code>
          <span style={{ width: 40, fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{step.px}</span>
          <span style={{ fontFamily: family, fontSize: step.px, lineHeight: 1.2 }}>
            The quick brown fox jumps over the lazy dog.
          </span>
        </div>
      ))}
    </div>
  );
}

export const Scale_: Story = {
  name: "Scale",
  render: () => <Scale />,
  parameters: {
    docs: {
      source: {
        code: `/* Typography tokens */
--rumahl-ui-typography-family: ...;   /* UI face */
--rumahl-ui-typography-display: ...;  /* statement face */

/* Component type roles — themes may replace the role, not every use. */
--type-chrome-size: 12px;
--type-label-size: 12px;
--type-compact-size: 12.5px;
--type-body-size: 13px;
--type-window-title-size: 15px;
--type-title-size: 30px;

/* 12px is a floor, not a suggestion. */`
      }
    }
  }
};
