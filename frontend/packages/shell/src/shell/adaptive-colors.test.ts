import { describe, expect, test } from "vitest";
import { contrastRatio } from "@rumahl/ui/color";
import { adaptiveInk, adaptiveMuted } from "./adaptive-colors";

describe("adaptive ink tones", () => {
  test("uses a softer light ink instead of pure white on dark glass", () => {
    const background = "#0d1c22";
    const ink = adaptiveInk(background);
    expect(ink).not.toBe("#ffffff");
    expect(contrastRatio(ink, background).ratio).toBeGreaterThanOrEqual(5);
  });

  test("uses a softer dark ink instead of pure black on light glass", () => {
    const background = "#eef2f1";
    const ink = adaptiveInk(background);
    expect(ink).not.toBe("#0b0d12");
    expect(contrastRatio(ink, background).ratio).toBeGreaterThanOrEqual(5);
  });

  test("keeps muted text readable without forcing the extreme tones", () => {
    for (const background of ["#0d1c22", "#eef2f1"]) {
      const muted = adaptiveMuted(background);
      expect(contrastRatio(muted, background).ratio).toBeGreaterThanOrEqual(4.5);
    }
  });
});

import { decideInk, ensureReadableTextColor, worstContrast } from "./adaptive-colors";
import { hexToHsl } from "@rumahl/ui/color";

describe("adaptive decisions", () => {
  test("keeps a readable tone instead of switching on a tiny improvement", () => {
    const backgrounds = ["#4a5357", "#5b666b"];
    const current = "#ffffff";
    const decision = decideInk(backgrounds, current);
    expect(decision.color).toBe(current);
    expect(decision.currentRatio).toBeGreaterThanOrEqual(5);
  });

  test("switches when the current tone falls below the safe range", () => {
    const decision = decideInk(["#f2f4f3", "#ffffff"], "#ffffff");
    expect(decision.color).not.toBe("#ffffff");
    expect(worstContrast(decision.color, ["#f2f4f3", "#ffffff"])).toBeGreaterThan(3);
  });

  test("preserves the hue of future coloured text while raising contrast", () => {
    const requested = "#e5484d";
    const dark = ensureReadableTextColor(["#101820"], requested);
    expect(dark.ratio).toBeGreaterThanOrEqual(4.5);
    expect(Math.abs(hexToHsl(dark.color)[0] - hexToHsl(requested)[0])).toBeLessThan(2);
  });

  test("uses the least favourable sample, not an average", () => {
    expect(worstContrast("#ffffff", ["#000000", "#ffffff"])).toBe(1);
    expect(worstContrast("#ffffff", ["#000000", "#202020"])).toBeGreaterThan(10);
  });
});

