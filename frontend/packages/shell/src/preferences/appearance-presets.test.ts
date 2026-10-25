import { describe, expect, test } from "vitest";
import { appearancePresets, applyLook, defaultLook, isLookActive } from "./appearance-presets";
import { emptyAppearance } from "./Workspace";

describe("appearance presets", () => {
  test("the default look is the lime canvas appearance", () => {
    expect(defaultLook).toMatchObject({
      seed: "#beff2d",
      mode: null,
      glassEngine: "canvas",
      animations: false
    });
    expect(defaultLook.tokens["typography.family"]).toContain("Manrope");
  });

  test("the neutral look restores the theme defaults", () => {
    const neutral = appearancePresets.find((preset) => preset.id === "neutral")!;
    expect(neutral.appearance).toEqual({ ...emptyAppearance, tokens: {} });
    expect(isLookActive({ ...emptyAppearance, tokens: {} }, neutral.appearance)).toBe(true);
  });

  test("looks are only active when the material and tokens match", () => {
    expect(isLookActive(defaultLook, defaultLook)).toBe(true);
    expect(isLookActive({ ...defaultLook, seed: "#0b84ff" }, defaultLook)).toBe(false);
    expect(isLookActive({ ...defaultLook, tokens: { ...defaultLook.tokens, "material.blur": "8px" } }, defaultLook)).toBe(false);
  });

  test("applying a look keeps the user's light/dark choice and wallpaper", () => {
    const current = {
      ...defaultLook,
      mode: "dark" as const,
      tokens: { ...defaultLook.tokens, "texture.wallpaper": "photo:towner" }
    };
    const applied = applyLook(current, appearancePresets.find((preset) => preset.id === "ocean")!.appearance);
    expect(applied.mode).toBe("dark");
    expect(applied.tokens["texture.wallpaper"]).toBe("photo:towner");
    const light = { ...current, mode: "light" as const };
    expect(applyLook(light, appearancePresets.find((preset) => preset.id === "amber")!.appearance).mode).toBe("light");
  });

  test("looks stay active across light/dark and wallpapers", () => {
    expect(isLookActive({ ...defaultLook, mode: "light" }, defaultLook)).toBe(true);
    expect(isLookActive({ ...defaultLook, mode: "dark" }, defaultLook)).toBe(true);
    expect(isLookActive({ ...defaultLook, tokens: { ...defaultLook.tokens, "texture.wallpaper": "photo:towner" } }, defaultLook)).toBe(true);
  });
});
