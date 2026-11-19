import { describe, expect, test } from "vitest";
import { clampDesktopPosition, desktopPositionInBounds } from "./desktop-positions";

const bounds = { width: 800, height: 600 };
const footprint = { width: 96, height: 104 };

describe("clampDesktopPosition", () => {
  test("keeps an in-bounds position unchanged", () => {
    expect(clampDesktopPosition({ x: 100, y: 50 }, bounds, footprint)).toEqual({ x: 100, y: 50 });
    expect(desktopPositionInBounds({ x: 100, y: 50 }, bounds, footprint)).toBe(true);
  });

  test("clamps negative and oversized positions into view", () => {
    expect(clampDesktopPosition({ x: -86, y: -165 }, bounds, footprint)).toEqual({ x: 0, y: 0 });
    expect(clampDesktopPosition({ x: 10_000, y: 10_000 }, bounds, footprint)).toEqual({ x: 800 - 96, y: 600 - 104 });
    expect(desktopPositionInBounds({ x: -86, y: -165 }, bounds, footprint)).toBe(false);
  });

  test("never returns a negative maximum on a tiny container", () => {
    expect(clampDesktopPosition({ x: 5, y: 5 }, { width: 40, height: 40 }, footprint)).toEqual({ x: 0, y: 0 });
  });
});
