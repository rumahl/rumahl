import { describe, expect, it } from "vitest";
import { parseDesktopLayout } from "./desktop-layout";

describe("desktop layout storage", () => {
  it("sanitizes a stored arrangement and falls back to defaults", () => {
    expect(parseDesktopLayout({ order: ["a", "a", "b", 1, ""], hidden: ["c", "c"], widgets: false }))
      .toEqual({ order: ["a", "b"], hidden: ["c"], widgets: false });
    expect(parseDesktopLayout(null)).toEqual({ order: [], hidden: [], widgets: true });
    expect(parseDesktopLayout({ order: "nope", hidden: 5 })).toEqual({ order: [], hidden: [], widgets: true });
    expect(parseDesktopLayout("x".repeat(300))).toEqual({ order: [], hidden: [], widgets: true });
  });
});
