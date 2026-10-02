import { describe, expect, test } from "vitest";
import { RUMAHL_COLORS, contrastRatio, generateColorShades, themeColorValues, withAlpha } from "./color";

describe("rumahl colour system", () => {
  test("generates a primary table and a secondary palette", () => {
    const palette = generateColorShades(RUMAHL_COLORS.base);
    expect(palette.primary.light).toHaveLength(21);
    expect(palette.primary.dark).toHaveLength(21);
    expect(palette.secondary.light).toHaveLength(21);
    expect(palette.secondary.dark).toHaveLength(21);
    for (const shade of palette.primary.light) expect(shade.hex).toMatch(/^#[0-9A-F]{6}$/i);
  });

  test("computes WCAG contrast", () => {
    const black = contrastRatio("#000000", "#ffffff");
    expect(black.ratio).toBeCloseTo(21, 0);
    expect(black.aaa).toBe(true);
  });

  test("derives theme colour values from the seed", () => {
    const values = themeColorValues(RUMAHL_COLORS.base);
    expect(values.accent).toBe(RUMAHL_COLORS.base);
    expect(values.surface).toMatch(/^#[0-9a-f]{8}$/);
    expect(values.onAccent).toBe("#ffffff");
    expect(withAlpha("#28694c", 0.5)).toBe("#28694c80");
  });
});
