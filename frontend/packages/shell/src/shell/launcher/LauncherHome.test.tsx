import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test } from "vitest";
import { App } from "../../App";
import { demoSnapshot } from "../../demo/snapshot";

beforeEach(() => localStorage.setItem("rumahl.demo.shell-mode", "launcher"));

describe("launcher shell", () => {
  test("switches between launcher layouts and persists the choice", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    expect(await screen.findByRole("heading", { name: "Your apps. Your space." })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "App drawer" }));
    expect(document.querySelector(".launcher-view--drawer")).not.toBeNull();
    expect(JSON.parse(localStorage.getItem("rumahl.demo.workspace")!).launcherView).toBe("deck");
    fireEvent.click(screen.getByRole("button", { name: "Cards" }));
    expect(screen.getByRole("list")).toBeInTheDocument();
  });

  test("groups apps alphabetically in the Android app drawer", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    expect(await screen.findByRole("heading", { name: "Your apps. Your space." })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "App drawer" }));
    const list = document.querySelector(".drawer-list");
    expect(list).not.toBeNull();
    expect(screen.getByRole("heading", { name: "F" })).toBeInTheDocument();
    expect(within(list as HTMLElement).getByRole("button", { name: /Files/ })).toBeInTheDocument();
    expect(screen.getByText("3 apps")).toBeInTheDocument();
  });

  test("shows workspace folders and opens them", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({ version: 1, windows: [], folders: [{ id: "tools", name: "Tools", apps: ["files"] }] }));
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    await screen.findByRole("button", { name: "Tools" });
    expect(screen.queryByRole("link", { name: "Files" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Tools" }));
    expect(screen.getByRole("button", { name: "All apps" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Files" })).toBeInTheDocument();
  });
});
