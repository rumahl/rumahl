import { describe, expect, test } from "vitest";
import { defineTheme, getTheme, listThemes, registerTheme } from "./sdk";
import { defaultTokens } from "./tokens";
import { resolveVariants, sanitizeVariants } from "./variants";

describe("theme sdk", () => {
  test("merges partial tokens over the rumahl defaults", () => {
    const theme = defineTheme({ id: "com.example.dark", name: "Dark", tokens: { "color.accent": "#000000" } });
    expect(theme.tokens["color.accent"]).toBe("#000000");
    expect(theme.tokens["material.blur"]).toBe(defaultTokens["material.blur"]);
    expect(theme.variants.shellLayout).toBe("dock");
  });

  test("rejects invalid variant values and keeps the defaults", () => {
    expect(sanitizeVariants({ shellLayout: "hologram", launcherLayout: "drawer" }))
      .toEqual({ shellLayout: "dock", launcherLayout: "drawer", windowChrome: "standard" });
    expect(resolveVariants({ windowChrome: "compact" }).windowChrome).toBe("compact");
    expect(sanitizeVariants(null)).toEqual(resolveVariants());
  });

  test("registers and lists themes", () => {
    const theme = defineTheme({ id: "com.example.classic", name: "Classic", variants: { shellLayout: "taskbar", launcherLayout: "drawer" } });
    registerTheme(theme);
    expect(getTheme("com.example.classic")).toBe(theme);
    expect(listThemes().some((entry) => entry.id === "com.example.classic")).toBe(true);
  });
});
