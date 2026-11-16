import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { themeFromPackage, type ThemePackageManifest } from "@rumahl/ui/themes";

/** Loads a bundled theme package exactly like the installer does. */
function loadTheme(name: string) {
  const manifest = JSON.parse(
    readFileSync(`${process.cwd()}/themes/${name}.json`, "utf8")
  ) as ThemePackageManifest & { css?: string };
  return themeFromPackage(manifest, manifest.css);
}

describe("bundled theme packages", () => {
  test("core builds on Ground without Iris branding", () => {
    const theme = loadTheme("core");
    expect(theme.base).toBe("ground");
    expect(theme.tokens["color.accent"]).toBe("#1a73e8");
    expect(theme.tokens["typography.family"]).not.toContain("Inter");
    expect(theme.tokens["typography.display"]).not.toContain("Space Grotesk");
    expect(theme.tokens["shape.radius.window"]).toBe("0px");
    expect(theme.tokens["shape.radius.dock"]).toBe("0px");
    expect(theme.tokens["shape.radius.icon"]).toBe("0px");
    expect(theme.tokens["material.morphism"]).toBe("0");
  });

  test("terminal keeps its explicit Ground values and stylesheet", () => {
    const theme = loadTheme("terminal");
    expect(theme.base).toBe("ground");
    expect(theme.tokens["color.accent"]).toBe("#39ff14");
    expect(theme.tokens["typography.family"]).toContain("Courier New");
    expect(theme.tokens["shape.radius.window"]).toBe("0px");
    expect(theme.css).toContain("Terminal");
  });

  test("dashboard's declared shape tokens survive Ground resolution", () => {
    const theme = loadTheme("dashboard");
    expect(theme.base).toBe("ground");
    expect(theme.tokens["color.accent"]).toBe("#4f46e5");
    expect(theme.tokens["shape.radius.window"]).toBe("18px");
    expect(theme.tokens["shape.radius.dock"]).toBe("18px");
    expect(theme.tokens["shape.radius.icon"]).toBe("14px");
    expect(theme.tokens["typography.family"]).toContain("Manrope");
    expect(theme.css).toContain("dash__panel");
  });

  test("existing packages without a base stay on Iris", () => {
    const theme = loadTheme("reference");
    expect(theme.base).toBeUndefined();
    expect(theme.tokens["color.accent"]).toBe("#e5432d");
    expect(theme.tokens["typography.family"]).toContain("Sora");
    expect(theme.tokens["shape.radius.window"]).toBe("26px");
  });
});
