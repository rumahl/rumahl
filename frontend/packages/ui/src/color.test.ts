import { describe, expect, it } from "vitest";
import { WCAG, contrastRatio, ensureContrast, readablePair, themeColorValues } from "./color";

describe("contrast engine", () => {
  it("keeps a background that already meets the target", () => {
    expect(ensureContrast("#000000", "#ffffff", WCAG.AA)).toBe("#000000");
  });

  it("adjusts a background until the ink reaches the target", () => {
    const adjusted = ensureContrast("#808080", "#ffffff", WCAG.AA);
    expect(contrastRatio("#ffffff", adjusted).ratio).toBeGreaterThanOrEqual(WCAG.AA);
  });

  it("readablePair guarantees the target for a mid-tone", () => {
    const { ink, background } = readablePair("#9aa0a6", WCAG.AA);
    expect(contrastRatio(ink, background).ratio).toBeGreaterThanOrEqual(WCAG.AA);
  });

  it("leaves a dark seed's accent untouched", () => {
    expect(themeColorValues("#28694c").accent).toBe("#28694c");
  });

  it("guarantees on-accent contrast even for an awkward seed", () => {
    const values = themeColorValues("#7f7f7f", { contrast: { onAccent: WCAG.AA } });
    expect(contrastRatio(values.onAccent, values.accent).ratio).toBeGreaterThanOrEqual(WCAG.AA);
  });
});
