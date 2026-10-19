import type { Meta, StoryObj } from "@storybook/react-vite";
import { Window } from "@rumahl/ui/components";

const meta = {
  title: "Components/Window",
  component: Window,
  args: { title: "System settings", subtitle: "Personal · Default", focused: true, variant: "standard", children: null },
  argTypes: {
    focused: { control: "boolean" },
    maximized: { control: "boolean" },
    frameless: { control: "boolean" },
    flush: { control: "boolean" }
  },
  parameters: {
    layout: "centered",
    windowSurface: false,
    docs: {
      description: {
        component: `The native OS window frame: a 54px titlebar (title + subtitle on one line), the
wired minimize / maximize / close controls, and a content area on the window
glass. The caller owns dragging, resizing and stacking.

- \`focused\`, \`maximized\`, \`frameless\`, \`flush\` (no content padding).
- \`titlebar\` — a custom topbar renderer: rebuild the titlebar like Windows or
  macOS and place the wired \`controls\` wherever you want.

## Rules
- **Don't nest a window in a window** — the content is the app.
- Use \`frameless\` + \`titlebar\` for a fully bespoke shell.
- Wire \`onClose\` / \`onMinimize\` / \`onMaximize\` so the controls work.`
      }
    }
  }
} satisfies Meta<typeof Window>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <div style={{ width: 760 }}>
      <Window {...args} onClose={() => {}} onMinimize={() => {}} onMaximize={() => {}}>
        <div style={{ padding: 20, minHeight: 300 }}>Window content lives here.</div>
      </Window>
    </div>
  )
};

export const Unfocused: Story = {
  render: (args) => (
    <div style={{ width: 760 }}>
      <Window {...args} focused={false} onClose={() => {}} onMinimize={() => {}} onMaximize={() => {}}>
        <div style={{ padding: 20, minHeight: 300 }}>An unfocused window.</div>
      </Window>
    </div>
  )
};

export const Frameless: Story = {
  render: (args) => (
    <div style={{ width: 760 }}>
      <Window {...args} frameless>
        <div style={{ padding: 20, minHeight: 300 }}>Frameless (hosted app).</div>
      </Window>
    </div>
  )
};

/**
 * The app rebuilds the topbar itself (like a custom title bar on Windows or
 * macOS) and places the wired window controls where it wants them.
 */
export const CustomTopbar: Story = {
  name: "Custom topbar",
  render: (args) => (
    <div style={{ width: 760 }}>
      <Window
        {...args}
        onClose={() => {}}
        onMinimize={() => {}}
        onMaximize={() => {}}
        titlebar={({ controls }) => (
          <>
            <strong style={{ fontSize: 12, fontWeight: 700 }}>Files</strong>
            <nav style={{ display: "flex", gap: 2, marginLeft: 6 }}>
              {["File", "Edit", "View", "Go"].map((label) => (
                <button
                  key={label}
                  type="button"
                  style={{
                    border: 0,
                    background: "transparent",
                    color: "inherit",
                    font: "inherit",
                    fontSize: 12,
                    padding: "5px 8px",
                    borderRadius: 6,
                    cursor: "pointer"
                  }}
                >
                  {label}
                </button>
              ))}
            </nav>
            <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 6 }}>
              <span style={{ fontSize: 11, color: "var(--rumahl-ui-color-text-muted)" }}>Search</span>
              {controls}
            </div>
          </>
        )}
      >
        <div style={{ padding: 20, minHeight: 300 }}>
          The app owns the whole topbar and embeds the window controls in it.
        </div>
      </Window>
    </div>
  )
};
