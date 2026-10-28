import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

describe("menu bar", () => {
  test("opens the system menu and closes it with Escape", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(screen.getByRole("button", { name: "rumahl OS" }));
    const menu = screen.getByRole("menu");
    expect(within(menu).getByRole("menuitem", { name: "Settings" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Sign out" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
  });

  test("switches the shell mode from the view menu", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(screen.getByRole("button", { name: "View" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Launcher" }));
    expect(document.querySelector(".shell")).toHaveAttribute("data-shell-mode", "launcher");
  });

  test("switches the shell mode from the top-bar toggle", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    const group = document.querySelector(".topbar__mode") as HTMLElement;
    fireEvent.click(within(group).getByRole("button", { name: "Launcher" }));
    expect(document.querySelector(".shell")).toHaveAttribute("data-shell-mode", "launcher");
  });

  test("opens the control centre from the top bar", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(screen.getByRole("button", { name: "Control centre" }));
    const panel = screen.getByRole("dialog", { name: "Control centre" });
    expect(within(panel).getAllByRole("button").length).toBeGreaterThanOrEqual(4);
  });

  test("lists open windows and focuses one", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/app-manager" />);
    fireEvent.click(screen.getByRole("button", { name: "Window" }));
    const menu = screen.getByRole("menu");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "App manager" }));
    expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();
  });
});
