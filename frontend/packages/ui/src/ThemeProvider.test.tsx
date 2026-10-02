import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { ThemeProvider, useTheme } from "./ThemeProvider";
import { tokenVariable } from "./tokens";

function Probe() {
  const { theme, variants } = useTheme();
  return <output>{`${theme.name}:${variants.shellLayout}:${variants.launcherLayout}`}</output>;
}

describe("theme provider", () => {
  test("exposes the active theme and projects its tokens onto the root", () => {
    render(<ThemeProvider><Probe /></ThemeProvider>);
    expect(screen.getByText("rumahl:dock:springboard")).toBeInstanceOf(HTMLElement);
    expect(document.documentElement.style.getPropertyValue(tokenVariable("color.accent"))).toBe("#28694c");
  });
});
