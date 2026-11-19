import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, test } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

function maximizeFiles() {
  fireEvent.click(screen.getByRole("button", { name: /maximize files/i }));
}

describe("fullscreen chrome", () => {
  beforeEach(() => localStorage.clear());

  test("a maximized window tucks the top bar and dock by default", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    await screen.findByRole("region", { name: "Files" });
    maximizeFiles();
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-dock-auto-hide", "true"));
    expect(shell).toHaveAttribute("data-topbar-auto-hide", "true");
    expect(document.querySelector(".menubar.topbar")).toHaveClass("is-auto-hide");
  });

  test("keeps the top bar when the account preference disables it", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({
      version: 1,
      windows: [],
      folders: [],
      fullscreenChrome: { hideTopbar: false, hideDock: true }
    }));
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    await screen.findByRole("region", { name: "Files" });
    maximizeFiles();
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-dock-auto-hide", "true"));
    expect(shell).not.toHaveAttribute("data-topbar-auto-hide");
    expect(document.querySelector(".menubar.topbar")).not.toHaveClass("is-auto-hide");
  });

  test("keeps window bottoms above a visible dock when the account opts in", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({
      version: 1,
      windows: [],
      folders: [],
      fullscreenChrome: { hideTopbar: true, hideDock: false, dockClearance: true }
    }));
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    await screen.findByRole("region", { name: "Files" });
    maximizeFiles();
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-dock-clearance", "true"));
    expect(shell).not.toHaveAttribute("data-dock-auto-hide");
  });

  test("ignores the clearance while the dock hides itself", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({
      version: 1,
      windows: [],
      folders: [],
      fullscreenChrome: { hideTopbar: true, hideDock: true, dockClearance: true }
    }));
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    await screen.findByRole("region", { name: "Files" });
    maximizeFiles();
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-dock-auto-hide", "true"));
    expect(shell).not.toHaveAttribute("data-dock-clearance");
  });

  test("enables the clearance switch only while the dock stays visible", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({
      version: 1,
      windows: [],
      folders: [],
      fullscreenChrome: { hideTopbar: true, hideDock: false, dockClearance: false }
    }));
    const first = render(<App snapshot={demoSnapshot} initialLocation="/settings/display/windows" />);
    expect(await screen.findByRole("switch", { name: "Keep windows above the dock" })).toBeEnabled();
    first.unmount();
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({
      version: 1,
      windows: [],
      folders: [],
      fullscreenChrome: { hideTopbar: true, hideDock: true, dockClearance: false }
    }));
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display/windows" />);
    expect(await screen.findByRole("switch", { name: "Hide dock" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Keep windows above the dock" })).toBeDisabled();
  });

  test("an open launcher app releases the dock row", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    await screen.findByRole("region", { name: "Files" });
    fireEvent.click(screen.getByRole("button", { name: "rumahl OS" }));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitemradio", { name: "Launcher" }));
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-shell-mode", "launcher"));
    expect(shell).toHaveAttribute("data-dock-auto-hide", "true");
    expect(shell).toHaveAttribute("data-topbar-auto-hide", "true");
  });
});
