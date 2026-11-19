import { expect, test } from "vitest";
import { isLaunching, setLaunching } from "./launching";

test("tracks which apps are launching", () => {
  expect(isLaunching("com.rumahl.devapp")).toBe(false);
  setLaunching("com.rumahl.devapp", true);
  expect(isLaunching("com.rumahl.devapp")).toBe(true);
  setLaunching("com.rumahl.devapp", false);
  expect(isLaunching("com.rumahl.devapp")).toBe(false);
});
