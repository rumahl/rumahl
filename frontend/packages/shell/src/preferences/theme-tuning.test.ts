import wallpaperUrl from "../assets/monstera.jpg";
import { expect, test } from "vitest";
import { defaultTheme } from "@rumahl/ui";
import { applyTuning, enableGlass, wallpaperSeed } from "./theme-tuning";
import { emptyAppearance, parseAppearance } from "./Workspace";

test("automatic colour survives parsing and follows wallpaper changes", () => {
  const appearance = parseAppearance({ ...emptyAppearance, autoColor: true });
  const first = applyTuning(defaultTheme.tokens, appearance);
  expect(first["color.accent"]).toBe("#0c1619");
  expect(wallpaperSeed(`url(${wallpaperUrl})`, "#ffffff")).toBe("#0c1619");
  const second = applyTuning(defaultTheme.tokens, { ...appearance, tokens: { "texture.wallpaper": "linear-gradient(160deg, #000000, #ffffff)" } });
  expect(second["color.accent"]).toBe("#808080");
  expect(wallpaperSeed("none", "#123456")).toBe("#123456");
  const manual = applyTuning(defaultTheme.tokens, { ...appearance, autoColor: false, seed: "#0078ff" });
  expect(manual["color.accent"]).toBe("#0078ff");
});

test("explicit glass activation recovers from solid and automatic reduction", () => {
  const solid = { ...emptyAppearance, glassEnabled: false, glassReduced: true, tokens: { "material.opacity": "1", "material.morphism": "0", "material.blur": "0px", "color.accent": "#abcdef" } };
  const next = enableGlass(solid);
  expect(next.glassEnabled).toBe(true);
  expect(next.glassReduced).toBe(false);
  expect(next.tokens["material.opacity"]).toBe("0.72");
  expect(next.tokens["material.morphism"]).toBe("1");
  expect(next.tokens["material.blur"]).toBe("12px");
  expect(next.tokens["color.accent"]).toBe("#abcdef");
  expect(solid.tokens["material.opacity"]).toBe("1");
});
test("glass activation supplies transparent defaults but preserves custom glass values", () => {
  expect(enableGlass(emptyAppearance).tokens["material.opacity"]).toBe("0.72");
  const tokens = { "material.opacity": "0.85", "material.morphism": "0.6", "material.blur": "18px" };
  expect(enableGlass({ ...emptyAppearance, tokens }).tokens).toEqual(tokens);
});
