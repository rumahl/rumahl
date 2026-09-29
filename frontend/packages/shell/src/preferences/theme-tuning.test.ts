import wallpaperUrl from "../assets/monstera.jpg";
import { expect, test } from "vitest";
import { defaultTheme } from "@rumahl/ui";
import { applyTuning, wallpaperSeed } from "./theme-tuning";
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
