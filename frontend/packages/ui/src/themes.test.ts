import { describe, expect, test } from "vitest";
import { readFileSync } from "node:fs";
import { defaultTheme, normalizeThemeBase, rumahlTheme, themeFromPackage, themes } from "./themes";
import { defaultTokens } from "./tokens";

const packageFile = (name: string) => readFileSync(`../../../examples/themes/hallows/${name}`, "utf8");
const classicFile = (name: string) => readFileSync(`../../../examples/themes/classic/${name}`, "utf8");
const classicTheme = themeFromPackage(JSON.parse(classicFile("theme.json")), classicFile("styles.css"));
const hallowsTheme = themeFromPackage(
  JSON.parse(packageFile("theme.json")),
  packageFile("styles.css"),
  packageFile("code.js")
);

describe("default themes", () => {
  test("rumahl is the default dock/springboard theme", () => {
    expect(defaultTheme).toBe(rumahlTheme);
    expect(defaultTheme.variants).toEqual({ shellLayout: "dock", launcherLayout: "springboard", windowChrome: "standard" });
  });

  test("classic fixture swaps structure and skin", () => {
    expect(classicTheme.variants.shellLayout).toBe("taskbar");
    expect(classicTheme.variants.launcherLayout).toBe("drawer");
    expect(classicTheme.tokens["material.blur"]).toBe("0px");
    expect(classicTheme.tokens["material.morphism"]).toBe("0");
    expect(rumahlTheme.tokens["material.morphism"]).toBe("1");
    const morphism = rumahlTheme.parameters.find((parameter) => parameter.kind === "range" && parameter.token === "material.morphism");
    expect(morphism && morphism.kind === "range" ? morphism.min : null).toBe(0);
    expect(classicTheme.tokens["shape.radius.dock"]).not.toBe(rumahlTheme.tokens["shape.radius.dock"]);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "color")).toBe(true);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "choice")).toBe(true);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "range")).toBe(true);
    expect(rumahlTheme.modes).toEqual(["light", "dark"]);
    expect(classicTheme.modes).toEqual(["light", "dark"]);
    const accent = rumahlTheme.parameters.find((parameter) => parameter.kind === "color" && parameter.seed === true);
    expect(accent && accent.kind === "color" ? accent.presets?.length ?? 0 : 0).toBeGreaterThanOrEqual(5);
    expect(classicTheme.parameters).not.toEqual(rumahlTheme.parameters);
    expect(themes).not.toContain(classicTheme);
  });
});

test("Iris keeps its established token identity", () => {
  expect(defaultTheme.tokens["color.accent"]).toBe("#28694c");
  expect(defaultTheme.tokens["typography.family"]).toContain("Inter");
  expect(defaultTheme.tokens["typography.display"]).toContain("Space Grotesk");
  expect(defaultTheme.tokens["shape.radius.window"]).toBe("23px");
  expect(defaultTheme.tokens["shape.radius.dock"]).toBe("28px");
  expect(defaultTheme.tokens["shape.radius.icon"]).toBe("18px");
  expect(defaultTheme.tokens["typography.size.floor"]).toBe("12px");
  expect(defaultTheme.tokens["typography.size.metadata"]).toBe("12px");
});

test("Ground defaults are brand-independent", () => {
  const ground = themeFromPackage({ id: "com.example.ground", name: "Ground" , base: "ground" });
  expect(ground.tokens["color.accent"]).not.toBe(defaultTokens["color.accent"]);
  expect(ground.tokens["typography.family"]).not.toContain("Inter");
  expect(ground.tokens["typography.display"]).not.toContain("Space Grotesk");
  expect(ground.tokens["shape.radius.window"]).toBe("0px");
  expect(ground.tokens["shape.radius.dock"]).toBe("0px");
  expect(ground.tokens["shape.radius.icon"]).toBe("0px");
  expect(ground.tokens["material.morphism"]).toBe("0");
});

test("Ground themes can opt out of generated light/dark palettes", () => {
  const fixed = themeFromPackage({
    id: "com.example.fixed",
    name: "Fixed",
    base: "ground",
    paletteTuning: false
  });
  expect(fixed.paletteTuning).toBe(false);
  const automatic = themeFromPackage({ id: "com.example.auto", name: "Auto", base: "ground" });
  expect(automatic.paletteTuning).toBeUndefined();
});

test("Ground themes can replace neutral defaults without losing their own identity", () => {
  const ground = themeFromPackage({
    id: "com.example.ground-custom",
    name: "Ground custom",
    base: "ground",
    tokens: {
      "color.accent": "#ff5500",
      "typography.family": "Georgia, serif",
      "shape.radius.window": "12px"
    }
  });
  expect(ground.tokens["color.accent"]).toBe("#ff5500");
  expect(ground.tokens["typography.family"]).toBe("Georgia, serif");
  expect(ground.tokens["shape.radius.window"]).toBe("12px");
  expect(ground.tokens["shape.radius.dock"]).toBe("0px");
});

test("legacy clean manifests receive the Ground foundation", () => {
  const legacy = themeFromPackage({ id: "com.example.legacy-clean", name: "Legacy clean", base: "clean" });
  expect(legacy.base).toBe("ground");
  expect(legacy.tokens["color.accent"]).not.toBe(defaultTokens["color.accent"]);
  expect(legacy.tokens["typography.family"]).not.toContain("Inter");
});

test("normalizes Iris and Ground theme bases, including legacy manifests", () => {
  expect(normalizeThemeBase("iris")).toBe("iris");
  expect(normalizeThemeBase("rumahl")).toBe("iris");
  expect(normalizeThemeBase("ground")).toBe("ground");
  expect(normalizeThemeBase("clean")).toBe("ground");
  expect(themeFromPackage({ id: "com.example.legacy", name: "Legacy", base: "clean" }).base).toBe("ground");
  expect(themeFromPackage({ id: "com.example.native", name: "Native", base: "rumahl" }).base).toBe("iris");
});

test("Iris owns the curated preset families; other themes do not inherit them", () => {
  expect(rumahlTheme.presets).toEqual({ look: true, material: true });
  expect(classicTheme.presets).toBeUndefined();
  expect(hallowsTheme.presets).toBeUndefined();
});

test("hallows is a dark-only candle-lit theme with its own stylesheet", () => {
  // Hallows ships as a package and is not part of the built-in registry.
  expect(themes).not.toContain(hallowsTheme);
  expect(themes.map((theme) => theme.id)).toEqual(["com.rumahl.default"]);
  expect(hallowsTheme.modes).toEqual(["dark"]);
  expect(hallowsTheme.id).toBe("com.rumahl.hallows");
  expect(hallowsTheme.tokens["color.canvas.background"]).toBe("#020101");
  expect(hallowsTheme.tokens["color.accent"]).toBe("#c81d1d");
  expect(hallowsTheme.tokens["texture.wallpaper"]).toBe("default");
  expect(hallowsTheme.tokens["effect.dread"]).toBe("45");
  expect(hallowsTheme.locked).toBe(true);
  const dread = hallowsTheme.parameters.find((parameter) => parameter.id === "dread");
  expect(dread?.kind).toBe("range");
  expect(dread?.kind === "range" ? [dread.min, dread.max] : []).toEqual([0, 100]);
  expect(hallowsTheme.parameters.filter((parameter) => parameter.kind === "choice").map((parameter) => parameter.id))
    .toEqual(["blood", "blackout", "red", "flicker", "glitch", "vhs", "signal", "presence", "unstable"]);
  expect(hallowsTheme.tokens["effect.blood"]).toBe("1");
  expect(hallowsTheme.tokens["typography.display"]).toContain("Fraunces");
  // The flicker is theme-owned CSS, and it must obey the motion switch.
  const css = readFileSync("../../../examples/themes/hallows/styles.css", "utf8");
  expect(css).toContain("hallows-stutter");
  expect(css).toContain("hallows-blackout");
  expect(css).toContain("hallows-red");
  expect(css).toContain("hallows-lights");
  expect(css).toContain("hallows-scanlines");
  expect(css).toContain("hallows-glitch");
  expect(css).toContain("hallows-presence");
  expect(css).toContain("hallows-unstable-bar");
  expect(css).toContain("prefers-reduced-motion: no-preference");
  expect(hallowsTheme.variants).toEqual(rumahlTheme.variants);
});
