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
    expect(classicTheme.tokens["shape.radius-dock"]).not.toBe(rumahlTheme.tokens["shape.radius-dock"]);
    expect(themes).toContain(classicTheme);
  });
});
