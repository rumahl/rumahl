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
    expect(JSON.parse(localStorage.getItem("rumahl.desktop.layout.v1")!).hidden).toContain("files");
  });

  test("finds and opens an app from the desktop search", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(document.querySelector(".desktop-search")!);
    const palette = screen.getByRole("dialog", { name: "Command palette" });
    fireEvent.change(within(palette).getByRole("textbox", { name: "Search commands" }), { target: { value: "Files" } });
    fireEvent.click(within(palette).getByRole("button", { name: /Files/ }));
    expect(screen.getByRole("region", { name: "Files" })).toBeInTheDocument();
  });

  test("opens a workspace folder from the desktop", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({ version: 1, windows: [], folders: [{ id: "tools", name: "Tools", apps: ["files"] }] }));
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(await screen.findByRole("button", { name: "Tools" }));
    expect(await screen.findByRole("link", { name: "Files" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "App manager" })).toBeNull();
  });
});
