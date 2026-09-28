import { describe, expect, test } from "vitest";
import { classicTheme, defaultTheme, rumahlTheme, themes } from "./themes";

describe("default themes", () => {
  test("rumahl is the default dock/springboard theme", () => {
    expect(defaultTheme).toBe(rumahlTheme);
    expect(defaultTheme.variants).toEqual({ shellLayout: "dock", launcherLayout: "springboard", windowChrome: "standard" });
  });

  test("classic fixture swaps structure and skin", () => {
    expect(classicTheme.variants.shellLayout).toBe("taskbar");
    expect(classicTheme.variants.launcherLayout).toBe("drawer");
    expect(classicTheme.tokens["material.blur"]).toBe("0px");
    expect(classicTheme.tokens["shape.radius.dock"]).not.toBe(rumahlTheme.tokens["shape.radius.dock"]);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "color")).toBe(true);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "choice")).toBe(true);
    expect(rumahlTheme.parameters.some((parameter) => parameter.kind === "range")).toBe(true);
    expect(rumahlTheme.modes).toEqual(["light", "dark"]);
    expect(classicTheme.modes).toEqual(["light", "dark"]);
    const accent = rumahlTheme.parameters.find((parameter) => parameter.kind === "color" && parameter.seed === true);
    expect(accent && accent.kind === "color" ? accent.presets?.length ?? 0 : 0).toBeGreaterThanOrEqual(5);
    expect(classicTheme.parameters).not.toEqual(rumahlTheme.parameters);
    expect(themes).toContain(classicTheme);
  });
});
