import { describe, expect, test } from "vitest";
import { glassColorTint } from "./glass";

describe("glass tint", () => {
  test("keeps the dock/widget tint distinct in light and dark schemes", () => {
    const light = glassColorTint(true, null, "#28694c", "light");
    const dark = glassColorTint(true, null, "#28694c", "dark");
    expect(light).toMatch(/^rgba\(/);
    expect(dark).toMatch(/^rgba\(/);
    expect(light).not.toBe(dark);
  });

  test("a custom coloured-glass tint overrides the accent", () => {
    const accent = glassColorTint(true, null, "#28694c", "light");
    const custom = glassColorTint(true, "#ff0000", "#28694c", "light");
    expect(custom).not.toBe(accent);
  });

  test("disabling coloured glass still follows the scheme but ignores the custom colour", () => {
    const red = glassColorTint(false, "#ff0000", "#28694c", "light");
    const blue = glassColorTint(false, "#0000ff", "#28694c", "light");
    const dark = glassColorTint(false, "#ff0000", "#28694c", "dark");
    expect(red).toBe(blue);
    expect(red).not.toBe(dark);
  });
});
