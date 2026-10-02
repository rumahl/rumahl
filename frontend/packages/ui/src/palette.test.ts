import { describe, expect, test } from "vitest";
import { contrastRatio, themeColorValues } from "./color";
import { paletteTokens } from "./palette";

describe("palette tokens", () => {
  test("keeps light surfaces neutral and only tints the accent", () => {
    const tokens = paletteTokens("#ff0000");
    const colors = themeColorValues("#ff0000");
    expect(tokens["color.accent"]).toBe("#ff0000");
    expect(tokens["color.accent.strong"]).toBe(colors.accentStrong);
    expect(tokens["color.on.accent"]).toBe(colors.onAccent);
    expect(tokens["color.surface.strong"]).toBe("#ffffff");
    expect(tokens["color.canvas.background"]).toBe("#c9c9ce");
  });

  test("derives distinct, readable dark materials from the colour engine", () => {
    const green = paletteTokens("#28694c", { mode: "dark" });
    const blue = paletteTokens("#0078ff", { mode: "dark" });
    expect(green["color.canvas.background"]).not.toBe(blue["color.canvas.background"]);
    expect(green["color.canvas.background"]).not.toBe(paletteTokens("#28694c")["color.canvas.background"]);
    for (const tokens of [green, blue]) {
      expect(contrastRatio(tokens["color.window.titlebar.foreground"]!, tokens["color.window.titlebar.background"]!).aa).toBe(true);
      expect(contrastRatio(tokens["color.text.primary"]!, tokens["color.panel.background"]!).aa).toBe(true);
      expect(contrastRatio(tokens["color.text.primary"]!, tokens["color.surface.strong"]!).aa).toBe(true);
      expect(tokens["color.canvas.background"]).not.toBe(tokens["color.panel.background"]);
    }
  });
});
