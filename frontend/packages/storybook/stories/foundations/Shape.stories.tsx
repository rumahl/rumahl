import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";

/**
 * # Shape
 *
 * Radius and size roles. Themes set the corner radius and icon sizes through
 * these tokens, so the whole shell follows without per-component changes.
 *
 * Radius roles: `--rumahl-ui-shape-radius-window` (windows), `…-dock`
 * (dock / floating islands), `…-icon` (app tiles), plus the smaller control
 * radii derived from the size scale.
 *
 * Ground starts square and exposes `--ground-radius` as its neutral fallback.
 * A Ground theme sets the shape tokens for window/dock/icon, the matching
 * recipe variable (`--window-radius`, `--menu-radius`, …) for one family, or
 * `--ground-radius` for the whole foundation.
 *
 * ## Rules
 *
 * - **Use the tokens, never a literal px** — a theme can round or square the
 *   whole UI in one place.
 * - **Round belongs to islands.** A single control / status item is a pill or
 *   circle; large surfaces (windows, panels) use the window radius.
 * - **Icon containers match the icon size token** so glyphs stay optically
 *   centred.
 */
const meta = {
  title: "Foundations/Shape",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const RADII = ["shape.radius.window", "shape.radius.dock", "shape.radius.icon"];
const SIZES = ["shape.icon.size", "shape.icon.size.large"];

function Specimen() {
  const { tokens } = useTheme();
  const token = (key: string) => (tokens as Record<string, string>)[key];
  return (
    <div style={{ display: "grid", gap: 28, padding: 24 }}>
      <section>
        <h2 style={{ fontSize: 14}}>Radius</h2>
        <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
          {RADII.map((key) => (
            <div key={key}>
              <div
                style={{
                  width: 96,
                  height: 72,
                  borderRadius: token(key),
                  background: "var(--rumahl-ui-color-surface-strong)",
                  boxShadow: "var(--control-inset, 0 1px 3px #0003)"
                }}
              />
              <div style={{ fontSize: 12, marginTop: 6 }}>{key}</div>
              <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{token(key)}</div>
            </div>
          ))}
        </div>
      </section>
      <section>
        <h2 style={{ fontSize: 14}}>Icon sizes</h2>
        <div style={{ display: "flex", gap: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
          {SIZES.map((key) => (
            <div key={key}>
              <div
                style={{
                  width: token(key),
                  height: token(key),
                  borderRadius: "30%",
                  background: "var(--rumahl-ui-icon-gradient)"
                }}
              />
              <div style={{ fontSize: 12, marginTop: 6 }}>{key}</div>
              <div style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>{token(key)}</div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

export const RadiusAndSize: Story = {
  render: () => <Specimen />,
  parameters: {
    docs: {
      source: {
        code: `/* Shape tokens — a theme can round or square the whole UI in one place. */
--rumahl-ui-shape-radius-window: 23px;  /* windows, panels */
--rumahl-ui-shape-radius-dock: 28px;    /* dock / floating islands */
--rumahl-ui-shape-radius-icon: 18px;    /* app tiles */

/* Ground only: the neutral foundation fallback for chrome without its own
   recipe variable. Set shape tokens above for per-family radii. */
--ground-radius: 0px;`
      }
    }
  }
};
