import { themeFromPackage } from "@rumahl/ui/themes";
import { loadThemePackage } from "../test/theme-packages";

const classicTheme = loadThemePackage("classic");
import { readFileSync } from "node:fs";
import { paletteTokens } from "@rumahl/ui/palette";
import { applyThemeTuning } from "./theme-tuning";
import wallpaperUrl from "../assets/monstera.jpg";
import { describe, expect, test } from "vitest";
import { defaultTheme } from "@rumahl/ui";
import { defineTheme } from "@rumahl/ui/sdk";
import { applyTuning, enableGlass, wallpaperSeed } from "./theme-tuning";
import { emptyAppearance, parseAppearance } from "./Workspace";

test("automatic colour survives parsing and follows wallpaper changes", () => {
  const appearance = parseAppearance({ ...emptyAppearance, autoColor: true });
  const first = applyTuning(defaultTheme.tokens, appearance);
  expect(first["color.accent"]).toBe("#0c1619");
  expect(wallpaperSeed(`url(${wallpaperUrl})`, "#ffffff")).toBe("#0c1619");
  const second = applyTuning(defaultTheme.tokens, {
    ...appearance,
    tokens: {
      "texture.wallpaper": "linear-gradient(160deg, #000000, #ffffff)",
    },
  });
  expect(second["color.accent"]).toBe("#808080");
  expect(wallpaperSeed("none", "#123456")).toBe("#123456");
  const manual = applyTuning(defaultTheme.tokens, {
    ...appearance,
    autoColor: false,
    seed: "#0078ff",
  });
  expect(manual["color.accent"]).toBe("#0078ff");
});

test("paletteTuning false keeps a Ground theme's authored scheme tokens", () => {
  const fixed = defineTheme({
    id: "com.test.fixed",
    name: "Fixed",
    base: "ground",
    modes: ["light", "dark"],
    paletteTuning: false,
    tokens: {
      "color.panel.background": "#fafafa",
      "color.canvas.background": "#ffffff",
      "color.text.primary": "#111111",
      "color.accent": "#ff5500"
    }
  });
  const tuned = applyThemeTuning(fixed, { ...emptyAppearance, mode: null }, "dark");
  expect(tuned["color.panel.background"]).toBe("#fafafa");
  expect(tuned["color.canvas.background"]).toBe("#ffffff");
  expect(tuned["color.text.primary"]).toBe("#111111");

  const automatic = applyThemeTuning(
    { ...fixed, paletteTuning: true },
    { ...emptyAppearance, mode: null },
    "dark"
  );
  expect(automatic["color.panel.background"]).not.toBe("#fafafa");
});

test("auto mode follows the system dark palette for glass themes", () => {
  const light = applyTuning(defaultTheme.tokens, { ...emptyAppearance, mode: null }, "light");
  const dark = applyTuning(defaultTheme.tokens, { ...emptyAppearance, mode: null }, "dark");
  const darkPalette = paletteTokens(defaultTheme.tokens["color.accent"], { mode: "dark" });
  expect(dark["color.canvas.background"]).toBe(darkPalette["color.canvas.background"]);
  expect(dark["color.canvas.background"]).not.toBe(light["color.canvas.background"]);
  const explicitLight = applyTuning(defaultTheme.tokens, { ...emptyAppearance, mode: "light" }, "dark");
  expect(explicitLight["color.canvas.background"]).toBe(paletteTokens(defaultTheme.tokens["color.accent"], { mode: "light" })["color.canvas.background"]);
});

test("explicit glass activation recovers from solid and automatic reduction", () => {
  const solid = {
    ...emptyAppearance,
    glassEnabled: false,
    glassReduced: true,
    tokens: {
      "material.opacity": "1",
      "material.morphism": "0",
      "material.blur": "0px",
      "color.accent": "#abcdef",
    },
  };
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
  const tokens = {
    "material.opacity": "0.85",
    "material.morphism": "0.6",
    "material.blur": "18px",
  };
  expect(enableGlass({ ...emptyAppearance, tokens }).tokens).toEqual(tokens);
});

test("keeps Classic opaque despite saved glass settings and derives both modes from the palette", () => {
  for (const mode of ["light", "dark"] as const) {
    const tokens = applyThemeTuning(classicTheme, {
      ...emptyAppearance,
      autoColor: false,
      seed: "#0078ff",
      mode,
      tokens: {
        "material.opacity": "0.4",
        "material.morphism": "1",
        "material.blur": "30px",
      },
    });
    expect(tokens["color.accent"]).toBe(
      paletteTokens("#0078ff", { mode })["color.accent"],
    );
    expect(tokens["color.panel.background"]).toBe(
      paletteTokens("#0078ff", { mode })["color.panel.background"],
    );
    expect(tokens["material.opacity"]).toBe("1");
    expect(tokens["material.morphism"]).toBe("0");
    expect(tokens["material.blur"]).toBe("0px");
    expect(tokens["color.surface"]).toBe(tokens["color.surface.strong"]);
  }
});

describe("glass colour preferences", () => {
  test("round-trips a custom tint and the coloured-glass switch", () => {
    const parsed = parseAppearance({ ...emptyAppearance, glassColor: false, glassTint: "#ff5500" });
    expect(parsed.glassColor).toBe(false);
    expect(parsed.glassTint).toBe("#ff5500");
  });

  test("ignores malformed tints instead of persisting unsafe CSS", () => {
    const parsed = parseAppearance({ ...emptyAppearance, glassTint: "red; display:none" });
    expect(parsed.glassTint).toBeUndefined();
  });
});


const packageFile = (name: string) => readFileSync(`${process.cwd()}/../../../examples/themes/hallows/${name}`, "utf8");
const hallowsTheme = themeFromPackage(
  JSON.parse(packageFile("theme.json")),
  packageFile("styles.css"),
  packageFile("code.js")
);

test("a locked art direction ignores colour tuning but keeps effect switches", () => {
  const tuned = applyThemeTuning(hallowsTheme, {
    ...emptyAppearance,
    seed: "#00ff00",
    autoColor: false,
    mode: "light",
    tokens: { "color.accent": "#00ff00", "color.canvas.background": "#00ff00", "effect.blood": "0" }
  });
  expect(tuned["color.accent"]).toBe("#c81d1d");
  expect(tuned["color.canvas.background"]).toBe("#020101");
  expect(tuned["effect.blood"]).toBe("0");
});
