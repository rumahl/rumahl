import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
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

  test("switches the shell mode from the view section", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    // The system menus live in the logo dropdown now; the left island is the
    // logo (round) plus the focused app's quick actions.
    fireEvent.click(screen.getByRole("button", { name: "rumahl OS" }));
    const menu = screen.getByRole("menu");
    fireEvent.click(within(menu).getByRole("menuitemradio", { name: "Launcher" }));
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

  test("shows the system dark scheme as active and switches to light", async () => {
    const original = window.matchMedia;
    window.matchMedia = vi.fn((query: string) => ({
      matches: query.includes("prefers-color-scheme: dark"),
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn()
    })) as unknown as typeof window.matchMedia;
    try {
      render(<App snapshot={demoSnapshot} initialLocation="/" />);
      await waitFor(() => {
        expect(document.querySelector(".shell")).toHaveAttribute("data-scheme", "dark");
      });
      fireEvent.click(screen.getByRole("button", { name: "Control centre" }));
      const panel = screen.getByRole("dialog", { name: "Control centre" });
      const darkMode = within(panel).getByRole("button", { name: "Dark mode" });
      expect(darkMode).toHaveAttribute("aria-pressed", "true");
      fireEvent.click(darkMode);
      await waitFor(() => {
        expect(document.querySelector(".shell")).toHaveAttribute("data-scheme", "light");
      });
    } finally {
      window.matchMedia = original;
    }
  });

  test("keeps transparency available after turning it off", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(screen.getByRole("button", { name: "Control centre" }));
    const panel = screen.getByRole("dialog", { name: "Control centre" });
    const transparency = within(panel).getByRole("button", { name: "Transparency" });
    expect(transparency).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(transparency);
    await waitFor(() => {
      expect(transparency).toHaveAttribute("aria-pressed", "false");
    });
    expect(transparency).not.toBeDisabled();
    fireEvent.click(transparency);
    await waitFor(() => {
      expect(transparency).toHaveAttribute("aria-pressed", "true");
    });
  });

  test("enabling transparency leaves performance mode instead of doing nothing", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    fireEvent.click(screen.getByRole("button", { name: "Control centre" }));
    const panel = screen.getByRole("dialog", { name: "Control centre" });
    const performance = within(panel).getByRole("button", { name: "Performance" });
    const transparency = within(panel).getByRole("button", { name: "Transparency" });
    fireEvent.click(performance);
    await waitFor(() => {
      expect(performance).toHaveAttribute("aria-pressed", "true");
      expect(transparency).toHaveAttribute("aria-pressed", "false");
    });
    fireEvent.click(transparency);
    await waitFor(() => {
      expect(performance).toHaveAttribute("aria-pressed", "false");
      expect(transparency).toHaveAttribute("aria-pressed", "true");
    });
  });

  test("lists open windows and focuses one", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    fireEvent.click(screen.getByRole("button", { name: "rumahl OS" }));
    const menu = screen.getByRole("menu");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Files" }));
    expect(screen.getByRole("region", { name: "Files" })).toBeInTheDocument();
  });

  test("shows only the round logo island while no app contributes quick actions", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    const island = document.querySelector(".topbar__island--left");
    expect(island).not.toHaveAttribute("data-quick");
    // The wordmark is visually hidden in the circle; the mark stays named.
    expect(within(island as HTMLElement).getByRole("button", { name: "rumahl OS" })).toBeInTheDocument();
  });

  test("carries the focused app's quick actions next to the logo", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
    // The Files window registers its quick actions; the focused window is
    // Files, so the left island stretches into a pill with those actions.
    await waitFor(() => {
      const island = document.querySelector(".topbar__island--left");
      expect(island).toHaveAttribute("data-quick");
      expect(within(island as HTMLElement).getByRole("button", { name: "New folder" })).toBeInTheDocument();
      expect(within(island as HTMLElement).getByRole("button", { name: "Refresh" })).toBeInTheDocument();
    });
  });
});
