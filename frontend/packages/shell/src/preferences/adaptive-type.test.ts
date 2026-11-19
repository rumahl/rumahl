import { expect, test } from "vitest";
import { adaptiveTypographicScale } from "./adaptive-type";

test("calibrates adaptive type to the reference desktop and clamps extremes", () => {
  expect(adaptiveTypographicScale(1440, 900)).toBe(1);
  expect(adaptiveTypographicScale(1920, 1080)).toBe(1.05);
  expect(adaptiveTypographicScale(2560, 1440)).toBe(1.11);
  expect(adaptiveTypographicScale(3840, 2160)).toBe(1.2);
  expect(adaptiveTypographicScale(1024, 600)).toBe(0.92);
  expect(adaptiveTypographicScale(320, 200)).toBe(0.9);
});
