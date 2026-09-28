import { describe, expect, test } from "vitest";
import { applyTokens, clearTokens, defaultTokens, tokenVariable } from "./tokens";

describe("design tokens", () => {
  test("projects tokens onto an element as CSS custom properties", () => {
    const root = document.createElement("div");
    applyTokens(root, defaultTokens);
    expect(tokenVariable("shape.radius-window")).toBe("--rumahl-ui-shape-radius-window");
    expect(root.style.getPropertyValue(tokenVariable("color.accent"))).toBe(defaultTokens["color.accent"]);
    clearTokens(root);
    expect(root.style.getPropertyValue(tokenVariable("color.accent"))).toBe("");
  });
});
