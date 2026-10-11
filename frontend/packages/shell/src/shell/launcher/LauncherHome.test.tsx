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
});
