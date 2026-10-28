import { render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { classicTheme } from "@rumahl/ui/themes";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

describe("theme-driven shell layout", () => {
  test("renders the classic taskbar instead of the dock", () => {
    render(<App snapshot={demoSnapshot} theme={classicTheme} initialLocation="/" />);
    expect(document.querySelector(".taskbar")).not.toBeNull();
    expect(document.querySelector(".os-dock")).toBeNull();
    const nav = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(nav).getByRole("link", { name: "Apps" })).toBeInTheDocument();
  });

  test("renders the launcher home", async () => {
    localStorage.setItem("rumahl.demo.shell-mode", "launcher");
    render(<App snapshot={demoSnapshot} theme={classicTheme} initialLocation="/" />);
    expect(await screen.findByRole("searchbox")).toBeInTheDocument();
    expect(document.querySelector(".launcher-board")).not.toBeNull();
  });
});
