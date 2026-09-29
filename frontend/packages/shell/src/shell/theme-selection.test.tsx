import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { paletteTokens } from "@rumahl/ui/palette";
import { getTheme } from "@rumahl/ui/sdk";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

describe("theme selection", () => {
  test("resolves built-in themes by id", () => {
    expect(getTheme("com.rumahl.classic")?.name).toBe("Classic");
    expect(getTheme("com.rumahl.default")?.name).toBe("rumahl");
  });

  test("applies the classic theme from the settings UI", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    const classic = await screen.findByRole("button", { name: "Classic" });
    fireEvent.click(classic);
    expect(classic).toHaveAttribute("aria-pressed", "true");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent")).toBe("#3465a4");
    expect(document.querySelector(".taskbar")).not.toBeNull();
    expect(document.querySelector(".os-dock")).toBeNull();
  });

  test("re-tints derivatives from the accent via the rumahl colour engine", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    fireEvent.change(await screen.findByLabelText("Accent colour"), { target: { value: "#ff0000" } });
    const expected = paletteTokens("#ff0000");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent")).toBe(expected["color.accent"]);
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent-strong")).toBe(expected["color.accent.strong"]);
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-surface-strong")).toBe(expected["color.surface.strong"]);
    fireEvent.change(screen.getByLabelText("Wallpaper"), { target: { value: "none" } });
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-texture-wallpaper")).toBe("none");
  });

  test("offers direct colour presets and a dark mode", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    fireEvent.click(await screen.findByRole("button", { name: "Red" }));
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent")).toBe("#e5484d");
    fireEvent.click(screen.getByRole("button", { name: "Dark" }));
    const dark = paletteTokens("#e5484d", { mode: "dark" });
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-surface-strong")).toBe(dark["color.surface.strong"]);
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-panel-background")).toBe(dark["color.panel.background"]);
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-window-titlebar-background")).toBe(dark["color.window.titlebar.background"]);
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-text-primary")).toBe(dark["color.text.primary"]);
    expect(dark["color.surface.strong"]).not.toBe("#000000");
    expect(JSON.parse(localStorage.getItem("rumahl.demo.workspace")!).appearance.mode).toBe("dark");
  });

  test("keeps text readable when a surface colour changes", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    fireEvent.change(await screen.findByLabelText("Panel colour"), { target: { value: "#000000" } });
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-surface-strong")).toBe("#000000");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-text-primary")).toBe("#ffffff");
  });

  test("lets the official theme tune transparency", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    const slider = await screen.findByRole("slider", { name: "Transparency" });
    fireEvent.change(slider, { target: { value: "0.5" } });
    expect(JSON.parse(localStorage.getItem("rumahl.demo.workspace")!).appearance.tokens["material.opacity"]).toBe("0.50");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-material-opacity")).toBe("0.50");
  });
  test("turns surface effects off atomically without changing the palette", async () => {
    const rendered = render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    fireEvent.click(await screen.findByRole("button", { name: "Red" }));
    const control = screen.getByRole("switch", { name: "Transparency effects" });
    fireEvent.click(control);
    expect(document.querySelector(".shell")).toHaveAttribute("data-material", "solid");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-material-blur")).toBe("0px");
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent")).toBe("#e5484d");
    rendered.unmount();
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    expect(await screen.findByRole("switch", { name: "Transparency effects" })).not.toBeChecked();
    fireEvent.click(screen.getByRole("switch", { name: "Transparency effects" }));
    expect(document.querySelector(".shell")).toHaveAttribute("data-material", "translucent");
  });

});
