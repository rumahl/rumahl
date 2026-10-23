import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test } from "vitest";
import { App } from "../../App";
import { demoSnapshot } from "../../demo/snapshot";

beforeEach(() => localStorage.setItem("rumahl.demo.shell-mode", "launcher"));

describe("launcher shell", () => {
  test("shows a greeting and the app grid", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    expect(await screen.findByRole("searchbox")).toBeInTheDocument();
    expect(await screen.findByRole("link", { name: /Files/ })).toBeInTheDocument();
  });

  test("shows workspace folders and opens them", async () => {
    localStorage.setItem("rumahl.demo.workspace", JSON.stringify({ version: 1, windows: [], folders: [{ id: "tools", name: "Tools", apps: ["files"] }] }));
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    await screen.findByRole("button", { name: "Tools" });
    expect(screen.queryByRole("link", { name: "Files" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Tools" }));
    const dialog = await screen.findByRole("dialog", { name: "Tools" });
    expect(within(dialog).getByRole("link", { name: "Files" })).toBeInTheDocument();
  });

  test("pages the springboard and switches pages with the dots", async () => {
    const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
    const height = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");
    Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, value: 60 });
    Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, value: 50 });
    try {
      render(<App snapshot={demoSnapshot} initialLocation="/" />);
      const pages = await screen.findByRole("navigation", { name: "Launcher pages" });
      const dots = within(pages).getAllByRole("button");
      expect(dots.length).toBeGreaterThan(1);
      expect(await screen.findByRole("link", { name: /Files/ })).toBeInTheDocument();
      fireEvent.click(dots[1]!);
      expect(await screen.findByRole("link", { name: /App manager/ })).toBeInTheDocument();
    } finally {
      if (width) Object.defineProperty(HTMLElement.prototype, "clientWidth", width);
      else Reflect.deleteProperty(HTMLElement.prototype, "clientWidth");
      if (height) Object.defineProperty(HTMLElement.prototype, "clientHeight", height);
      else Reflect.deleteProperty(HTMLElement.prototype, "clientHeight");
    }
  });
});
