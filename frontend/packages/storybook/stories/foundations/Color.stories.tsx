import type { Meta, StoryObj } from "@storybook/react-vite";
import { generateColorShades, useTheme } from "@rumahl/ui";

/**
 * # Colour
 *
 * Colour is a set of **roles**, never literals. Every role is a
 * `--rumahl-ui-*` token, so a custom theme restyles the whole UI by overriding
 * tokens — no component code changes required.
 *
 * ## How a palette is derived
 *
 * A theme provides **one accent seed** and a **mode** (light/dark).
 * `paletteTokens(seed, { mode })` derives the whole palette: the accent scale,
 * the neutral surfaces (canvas / panel / surface-strong), text roles and the
 * status colours. Change the seed and the entire UI follows.
 *
 * ## Contrast is enforced
 *
 * Text roles are corrected per surface with `ensureContrast` to meet WCAG
 * targets (AA for text on surfaces and on the accent, AAA for status, AA-large
 * for large text). `color.on.accent` is always nudged to be readable on the
 * accent, so a coloured button never ships with unreadable text.
 *
 * ## Rules
 *
 * - **Use roles, not hex.** Reach for `--rumahl-ui-color-text-primary`,
 *   `…-surface-strong`, `…-accent`, `…-on-accent`, etc. Never hard-code a colour
 *   in a component — a theme must be able to recolour everything.
 * - **Surfaces follow the accent.** Because they are derived from the seed, the
 *   window / sidebar / group surfaces stay in the same family. See *Surfaces*.
 * - **`on-*` tokens for layered content.** Text placed on an accent or status
 *   colour uses the matching `on-*` role, not a fixed white/black.
 * - **Status colours are semantic** (info / success / warning / danger) and
 *   match their auto contrast — do not repurpose them decoratively.
 * - **Prefer elevation and material over new colours** to separate layers;
 *   reserve strong colour for meaning and the accent.
 */
const meta = {
  title: "Foundations/Color",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "Colour tokens projected onto the document root as `--rumahl-ui-color-*`. " +
          "Use the Appearance/Material toolbars to see how they resolve."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Swatches() {
  const { tokens } = useTheme();
  const entries = Object.entries(tokens).filter(([key]) => key.startsWith("color."));
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: 14, padding: 24 }}>
      {entries.map(([key, value]) => (
        <div key={key} style={{ border: "1px solid var(--rumahl-ui-color-outline)", borderRadius: 12, overflow: "hidden" }}>
          <div style={{ height: 64, background: value }} />
          <div style={{ padding: "8px 10px" }}>
            <strong style={{ fontSize: 12}}>{key}</strong>
            <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{value}</div>
          </div>
        </div>
      ))}
    </div>
  );
}

/** The generated accent scale (shades of the seed colour). */
function Shades() {
  const { tokens } = useTheme();
  const seed = (tokens as Record<string, string>)["color.accent"] ?? "#28694c";
  const shades = generateColorShades(seed).primary.light;
  return (
    <div style={{ display: "grid", gap: 8, padding: 24 }}>
      {shades.map((shade) => (
        <div key={shade.shade} style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span style={{ width: 48, fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{shade.shade}</span>
          <span style={{ width: 140, height: 22, borderRadius: 6, background: shade.hex }} />
          <code style={{ fontSize: 12}}>{shade.hex}</code>
        </div>
      ))}
    </div>
  );
}

const SWATCH_CODE = `import { useTheme } from "@rumahl/ui";

export function Swatches() {
  const { tokens } = useTheme();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: 14 }}>
      {Object.entries(tokens)
        .filter(([key]) => key.startsWith("color."))
        .map(([key, value]) => (
          <div key={key} style={{ border: "1px solid var(--rumahl-ui-color-outline)", borderRadius: 12, overflow: "hidden" }}>
            <div style={{ height: 64, background: value }} />
            <div style={{ padding: "8px 10px" }}>
              <strong style={{ fontSize: 12}}>{key}</strong>
              <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{value}</div>
            </div>
          </div>
        ))}
    </div>
  );
}`;

export const Roles: Story = {
  render: () => <Swatches />,
  parameters: { docs: { source: { language: "tsx", code: SWATCH_CODE } } }
};
export const AccentScale: Story = {
  render: () => <Shades />,
  parameters: {
    docs: {
      source: {
        language: "tsx",
        code: `import { generateColorShades } from "@rumahl/ui";

// The accent scale is derived from the seed accent colour.
const shades = generateColorShades("#28694c").primary.light;
// -> [{ level: 50, hex: "…" }, …]`
      }
    }
  }
};
