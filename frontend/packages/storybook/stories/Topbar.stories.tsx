import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlMark } from "@rumahl/ui/components";
import { DesktopIcon, GridIcon, SearchIcon, SlidersIcon } from "@rumahl/ui/icons";
// The top bar is a shell surface; its stylesheet is imported here so the
// elements can be previewed without the running shell.
import "../../shell/src/shell/topbar.css";

/**
 * Preview of the top bar elements. The top bar is intentionally NOT part of the
 * app material system (Glass/Solid/Floating): it keeps its own frosted island
 * look and is never affected by `data-app-inputs` or the app input material.
 */
const meta = {
  title: "Shell/Topbar",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "The top bar is unaffected by the app material system. It is shown " +
          "here only as a visual reference for the shell chrome."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Topbar({ mode = "desktop" }: { mode?: "desktop" | "launcher" }) {
  return (
    <header className="menubar topbar" style={{ position: "relative" }}>
      <div className="topbar__island topbar__island--left">
        <nav className="menubar__menus" aria-label="Top bar">
          <div className="menubar__menu">
            <button className="menubar__item" type="button">
              <span className="menubar__brand brand" aria-label="rumahl OS">
                <RumahlMark />
              </span>
            </button>
          </div>
          {["Go", "View", "Window"].map((label) => (
            <div className="menubar__menu" key={label}>
              <button className="menubar__item" type="button">{label}</button>
            </div>
          ))}
        </nav>
      </div>
      <div className="topbar__island topbar__island--right">
        <button className="topbar__button menubar__spotlight" type="button" aria-label="Search">
          <SearchIcon />
        </button>
        <div className="topbar__anchor">
          <button className="topbar__button" type="button" aria-label="Control center">
            <SlidersIcon />
          </button>
        </div>
        <div className="topbar__mode" role="group" aria-label="Mode">
          <button className="topbar__button" type="button" aria-pressed={mode === "desktop"} aria-label="Desktop">
            <DesktopIcon />
          </button>
          <button className="topbar__button" type="button" aria-pressed={mode === "launcher"} aria-label="Launcher">
            <GridIcon />
          </button>
        </div>
        <time className="menubar__clock">Wed 11:19 AM</time>
        <div className="menubar__menu">
          <button className="menubar__item menubar__user" type="button" aria-label="Account">
            <span className="menubar__avatar" aria-hidden="true">D</span>
          </button>
        </div>
      </div>
    </header>
  );
}

export const Desktop: Story = { render: () => <Topbar mode="desktop" /> };
export const Launcher: Story = { render: () => <Topbar mode="launcher" /> };
