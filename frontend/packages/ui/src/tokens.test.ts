import { describe, expect, test } from "vitest";
import { applyTokens, clearTokens, defaultTokens, tokenVariable } from "./tokens";

describe("design tokens", () => {
  test("publishes the agreed type and shape contract", () => {
    expect(defaultTokens["typography.size.floor"]).toBe("12px");
    expect(defaultTokens["typography.size.metadata"]).toBe("12px");
    expect(defaultTokens["shape.radius.window"]).toBe("23px");
    expect(defaultTokens["shape.radius.dock"]).toBe("28px");
    expect(defaultTokens["shape.radius.icon"]).toBe("18px");
  });

  test("projects tokens onto an element as CSS custom properties", () => {
    const root = document.createElement("div");
    applyTokens(root, defaultTokens);
    expect(tokenVariable("shape.radius.window")).toBe("--rumahl-ui-shape-radius-window");
    expect(root.style.getPropertyValue(tokenVariable("color.accent"))).toBe(defaultTokens["color.accent"]);
    clearTokens(root);
    expect(root.style.getPropertyValue(tokenVariable("color.accent"))).toBe("");
  });
});
