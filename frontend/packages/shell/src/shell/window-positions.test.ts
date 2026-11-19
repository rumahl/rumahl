import { beforeEach, expect, it, vi } from "vitest";
import { readWindowRect, writeWindowRect } from "./window-positions";

beforeEach(() => localStorage.clear());
it("remembers each window independently across reopening", () => {
  const rect = { x: 123, y: 87, width: 640, height: 480 };
  writeWindowRect("app:files", rect);
  expect(readWindowRect("app:files")).toEqual(rect);
  expect(readWindowRect("settings")).toBeUndefined();
});
it("ignores corrupt and invalid saved rectangles", () => {
  for (const value of ["bad json", '{"x":0,"y":0,"width":-1,"height":400}', '{"x":"0","y":0,"width":500,"height":400}']) {
    localStorage.setItem("rumahl.window-position.v1:files", value);
    expect(readWindowRect("files")).toBeUndefined();
  }
});
it("continues when storage is blocked", () => {
  const spy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw Error("blocked"); });
  expect(() => writeWindowRect("files", { x: 0, y: 0, width: 500, height: 400 })).not.toThrow();
  spy.mockRestore();
});
