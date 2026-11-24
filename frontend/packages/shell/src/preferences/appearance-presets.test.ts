import { describe, expect, test } from "vitest";
import { appearancePresets, applyLook, irisLook, isLookActive } from "./appearance-presets";
import { emptyAppearance, parseAppearance } from "./Workspace";

describe("appearance presets", () => {
  test("Iris is the default look and restores the theme defaults", () => {
    expect(appearancePresets[0]).toMatchObject({ id: "iris", labelKey: "theme.look.iris" });
    expect(irisLook).toEqual({ ...emptyAppearance, tokens: {} });
    expect(isLookActive({ ...emptyAppearance, tokens: {} }, irisLook)).toBe(true);
  });

  test("no separate Aura identity remains", () => {
    expect(appearancePresets.some((preset) => preset.id === "aura")).toBe(false);
    expect(appearancePresets.some((preset) => String(preset.labelKey) === "theme.look.aura")).toBe(false);
  });

  test("the former lime look remains an optional tuning, not the default", () => {
    const lime = appearancePresets.find((preset) => preset.id === "lime");
    expect(lime?.appearance.seed).toBe("#beff2d");
    expect(lime?.appearance.tokens["typography.family"]).toContain("Manrope");
    expect(appearancePresets[0]?.id).toBe("iris");
  });

  test("looks are only active when the material and tokens match", () => {
    expect(isLookActive(irisLook, irisLook)).toBe(true);
    expect(isLookActive({ ...irisLook, seed: "#0b84ff" }, irisLook)).toBe(false);
    expect(isLookActive({ ...irisLook, tokens: { ...irisLook.tokens, "material.blur": "8px" } }, irisLook)).toBe(false);
  });

  test("applying a look keeps the user's light/dark choice and wallpaper", () => {
    const current = {
      ...irisLook,
      mode: "dark" as const,
      tokens: { ...irisLook.tokens, "texture.wallpaper": "photo:towner" }
    };
    const applied = applyLook(current, appearancePresets.find((preset) => preset.id === "ocean")!.appearance);
    expect(applied.mode).toBe("dark");
    expect(applied.tokens["texture.wallpaper"]).toBe("photo:towner");
    const light = { ...current, mode: "light" as const };
    expect(applyLook(light, appearancePresets.find((preset) => preset.id === "amber")!.appearance).mode).toBe("light");
  });

  test("an explicit glass technique enables glass unless the master switch is off", () => {
    expect(parseAppearance({ glassBackend: "svg" }).glassEnabled).toBe(true);
    expect(parseAppearance({ glassBackend: "svg", glassEnabled: false }).glassEnabled).toBe(false);
  });

  test("the mobile SVG navbar choice survives parsing and look changes", () => {
    const parsed = parseAppearance({ mobileNavSvgGlass: true, tokens: {} });
    expect(parsed.mobileNavSvgGlass).toBe(true);
    expect(parseAppearance({ mobileNavSvgGlass: "yes" }).mobileNavSvgGlass).toBeUndefined();
    const applied = applyLook(parsed, appearancePresets.find((preset) => preset.id === "ocean")!.appearance);
    expect(applied.mobileNavSvgGlass).toBe(true);
  });

  test("applying a look does not replace the chosen glass renderer", () => {
    const current = {
      ...irisLook,
      glassBackend: "svg" as const,
      glassQuality: "high" as const,
      glassEnabled: true,
      glassReduced: false
    };
    const applied = applyLook(current, appearancePresets.find((preset) => preset.id === "ocean")!.appearance);
    expect(applied.glassBackend).toBe("svg");
    expect(applied.glassQuality).toBe("high");
    expect(applied.glassEnabled).toBe(true);
    expect(applied.glassReduced).toBe(false);
  });

  test("looks stay active across light/dark and wallpapers", () => {
    const soft = { ...irisLook, tokens: { "iris.controls": "soft" } };
    expect(isLookActive(soft, irisLook)).toBe(true);
    expect(applyLook(soft, appearancePresets[1]!.appearance).tokens["iris.controls"]).toBe("soft");
    expect(isLookActive({ ...irisLook, mode: "light" }, irisLook)).toBe(true);
    expect(isLookActive({ ...irisLook, mode: "dark" }, irisLook)).toBe(true);
    expect(isLookActive({ ...irisLook, tokens: { ...irisLook.tokens, "texture.wallpaper": "photo:towner" } }, irisLook)).toBe(true);
  });
});
