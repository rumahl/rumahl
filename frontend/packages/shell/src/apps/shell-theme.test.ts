import { describe, expect, test } from "vitest";
import { readShellTheme } from "./shell-theme";

describe("readShellTheme", () => {
  test("maps the theme tokens into a bridge theme", () => {
    const theme = readShellTheme({
      "color.canvas.background": "#000000",
      "color.surface": "#111111",
      "color.text.primary": "#eeeeee",
      "color.text.muted": "#888888",
      "color.outline": "#333333",
      "color.accent": "#00ff00",
      "color.on.accent": "#000000"
    });
    expect(theme.palette).toEqual({
      background: "#000000", surface: "#111111", text: "#eeeeee",
      textMuted: "#888888", border: "#333333", accent: "#00ff00", onAccent: "#000000"
    });
    expect(theme.scheme).toBe("dark");
    expect(typeof theme.reducedMotion).toBe("boolean");
  });
});
