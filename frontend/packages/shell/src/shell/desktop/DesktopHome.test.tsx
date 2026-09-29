import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { App } from "../../App";
import { demoSnapshot } from "../../demo/snapshot";

describe("desktop shell", () => {
  test("opens the background context menu on right click", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.contextMenu(document.querySelector(".desktop-shortcuts")!);
    const menu = screen.getByRole("menu", { name: "Desktop menu" });
    expect(within(menu).getByRole("menuitem", { name: "Save arrangement" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Settings" })).toBeInTheDocument();
  });

  test("removes a shortcut from the desktop and keeps it recoverable", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.contextMenu(screen.getByRole("link", { name: "Files" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Remove from desktop" }));
    expect(screen.queryByRole("link", { name: "Files" })).toBeNull();
    expect(JSON.parse(localStorage.getItem("rumahl.demo.workspace")!).desktop.hidden).toContain("files");
  });

  test("renders installed apps from the snapshot without a catalog fetch", () => {
    render(<App snapshot={{ ...demoSnapshot, apps: [{ id: "com.example.notes", title: "Notes", launchable: true }] }} initialLocation="/" />);
    expect(screen.getByRole("link", { name: "Notes" })).toBeInTheDocument();
  });

  test("opens a workspace folder from the desktop", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({ version: 1, windows: [], folders: [{ id: "tools", name: "Tools", apps: ["files"] }] }));
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(await screen.findByRole("button", { name: "Tools" }));
    expect(await screen.findByRole("link", { name: "Files" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "App manager" })).toBeNull();
  });
});
