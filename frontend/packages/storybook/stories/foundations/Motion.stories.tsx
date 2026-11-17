import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTheme } from "@rumahl/ui";

/**
 * # Motion
 *
 * Motion roles. Durations and the spring easing are tokens so a theme (or a
 * reduced-motion preference) can tune or disable animation consistently.
 *
 * - `--motion-feedback-duration` (~120ms) — hover / focus colour changes
 * - `--motion-surface-duration` (~180ms) — surface enter/exit
 * - `--motion-spring-{bouncy,soft,snap}` — launcher, dock and window gestures
 *
 * ## Rules
 *
 * - **Animate state, not decoration** — hover, press, enter, exit. No looping
 *   animation in UI chrome.
 * - **Colour and opacity first.** Reach for transform/scale only for spatial
 *   changes (a window opening, a menu appearing).
 * - **Always honour `prefers-reduced-motion: reduce`** — transitions collapse to
 *   none; never gate meaning on an animation completing.
 * - **Keep it quick.** Fast feedback (≤120ms) for state; ~200–280ms for
 *   spatial entry.
 */
const meta = {
  title: "Foundations/Motion",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const KEYS = ["motion.duration.fast", "motion.duration", "motion.easing.spring"];

function Specimen() {
  const { tokens } = useTheme();
  const token = (key: string) => (tokens as Record<string, string>)[key];
  return (
    <div style={{ display: "grid", gap: 24, padding: 24, maxWidth: 640 }}>
      <dl style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "8px 16px", fontSize: 12, margin: 0 }}>
        {KEYS.map((key) => (
          <div key={key} style={{ display: "contents" }}>
            <dt style={{ color: "var(--rumahl-ui-color-text-muted)" }}>{key}</dt>
            <dd style={{ margin: 0 }}>{token(key)}</dd>
          </div>
        ))}
      </dl>
      <div
        style={{
          height: 12,
          borderRadius: 999,
          background: "var(--rumahl-ui-color-accent)",
          width: 40,
          boxShadow: "0 6px 18px -6px var(--rumahl-ui-color-accent)",
          animation: `rumahl-motion-demo ${token("motion.duration")} ${token("motion.easing.spring")} infinite alternate`
        }}
      />
      <p style={{ fontSize: 12, color: "var(--rumahl-ui-color-text-muted)" }}>
        The bar animates with the theme's duration and spring easing.
      </p>
      <style>{`@keyframes rumahl-motion-demo { to { width: 320px; } }`}</style>
    </div>
  );
}

export const Duration: Story = {
  render: () => <Specimen />,
  parameters: {
    docs: {
      source: {
        code: `/* Motion tokens — collapse under prefers-reduced-motion. */
--rumahl-ui-motion-duration-fast: 120ms;  /* hover / focus colour */
--rumahl-ui-motion-duration: 180ms;       /* surface state change */

/* Component motion roles — the override surface. */
--motion-feedback-duration: var(--rumahl-ui-motion-duration-fast, 120ms);
--motion-surface-duration: var(--rumahl-ui-motion-duration, 180ms);
--motion-hover-duration: 200ms;
--motion-easing-standard: cubic-bezier(0.2, 0.7, 0.2, 1);
--motion-spring-bouncy: cubic-bezier(0.34, 1.56, 0.64, 1);`
      }
    }
  }
};
