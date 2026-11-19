import { expect, test } from "vitest";
import { resetAppearanceGroup, resetColorOverrides, resetThemeParameters } from "./appearance-reset";
import type { AppearanceSettings } from "./Workspace";

const original: AppearanceSettings = {
  seed: "#123456", mode: "dark", adaptiveType: true, animations: false,
  performanceMode: true, glassBackend: "webgl", glassTint: "#abcdef", wallpaperMotion: false,
  tokens: { "typography.scale": "1.4", "typography.family": "Custom Sans", "material.opacity": "0.6", "color.text.primary": "#ffffff", "texture.wallpaper": "none", "layout.dock": "left" }
};

test("typography reset cannot change palette, wallpaper, glass or performance", () => {
  const next = resetAppearanceGroup(original, "typography");
  expect(next.adaptiveType).toBeUndefined();
  expect(next.tokens["typography.scale"]).toBeUndefined();
  expect(next).toMatchObject({ seed: original.seed, mode: "dark", glassBackend: "webgl", performanceMode: true });
  expect(next.tokens["texture.wallpaper"]).toBe("none");
  expect(next.tokens["typography.family"]).toBe("Custom Sans");
  expect(original.tokens["typography.scale"]).toBe("1.4");
});

test("surface, glass and performance resets remain independent", () => {
  const surface = resetAppearanceGroup(original, "surface");
  expect(surface.wallpaperMotion).toBeUndefined();
  expect(surface.tokens["material.opacity"]).toBeUndefined();
  expect(surface.glassTint).toBe(original.glassTint);
  expect(surface.performanceMode).toBe(true);
  const glass = resetAppearanceGroup(original, "glass");
  expect(glass.glassBackend).toBeUndefined();
  expect(glass.glassTint).toBeUndefined();
  expect(glass.tokens).toEqual(original.tokens);
  const performance = resetAppearanceGroup(original, "performance");
  expect(performance.performanceMode).toBeUndefined();
  expect(performance.animations).toBeUndefined();
  expect(performance.tokens).toEqual(original.tokens);
  expect(performance.glassBackend).toBe("webgl");
});

test("theme reset only clears declared options, including an explicitly declared seed", () => {
  const next = resetThemeParameters(original, [{ token: "layout.dock", kind: "choice" }]);
  expect(next.tokens["layout.dock"]).toBeUndefined();
  expect(next.tokens["texture.wallpaper"]).toBe("none");
  expect(next.tokens["typography.scale"]).toBe("1.4");
  expect(next.seed).toBe(original.seed);
  expect(resetThemeParameters(original, [{ token: "color.accent", kind: "color", seed: true }]).seed).toBeNull();
});

test("color overrides reset the displayed colors and mode, not other customizations", () => {
  const next = resetColorOverrides(original, ["color.text.primary"]);
  expect(next.mode).toBeNull();
  expect(next.tokens["color.text.primary"]).toBeUndefined();
  expect(next.tokens["typography.scale"]).toBe("1.4");
  expect(next.seed).toBe(original.seed);
});
