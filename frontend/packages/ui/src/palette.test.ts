import { describe, expect, test } from "vitest";
import { themeColorValues } from "./color";
import { paletteTokens } from "./palette";

describe("palette tokens", () => {
  test("projects the colour engine onto theme roles", () => {
    const tokens = paletteTokens("#ff0000");
    const colors = themeColorValues("#ff0000");
    expect(tokens["color.accent"]).toBe("#ff0000");
    expect(tokens["color.accent.strong"]).toBe(colors.accentStrong);
    expect(tokens["color.surface"]).toBe(colors.surface);
    expect(tokens["color.surface.strong"]).toBe(colors.surfaceStrong);
    expect(tokens["color.on.accent"]).toBe(colors.onAccent);
  });
});
