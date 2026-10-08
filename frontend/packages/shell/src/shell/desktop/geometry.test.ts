import { describe, expect, it } from "vitest";
import { centeredRect, constrainRect, placedRect, rectsIntersect } from "./geometry";
import { initialShellState, shellReducer } from "../../shell-state";
describe("desktop windows", () => {
  it("centers new windows and fits them to small desktops", () => {
    expect(centeredRect({ width: 1200, height: 800 })).toEqual({ x: 220, y: 130, width: 760, height: 540 });
    expect(centeredRect({ width: 320, height: 400 })).toEqual({ x: 0, y: 0, width: 320, height: 400 });
  });
  it("preserves supplied geometry and leaves new windows unpositioned until measurement", () => {
    const window = { id: "one", title: "One", subtitle: "" };
    expect(shellReducer(initialShellState, { type: "open-window", window }).windows[0]?.rect).toBeUndefined();
    const rect = { x: 120, y: 90, width: 500, height: 350 };
    expect(shellReducer(initialShellState, { type: "open-window", window: { ...window, rect } }).windows[0]?.rect).toEqual(rect);
  });
  it("keeps windows reachable on small displays and after resizing the viewport", () => {
    expect(constrainRect({ x: 900, y: 600, width: 760, height: 540 }, { width: 320, height: 400 })).toEqual({ x: 272, y: 352, width: 320, height: 400 });
    expect(constrainRect({ x: -30, y: -20, width: 1, height: 1 }, { width: 1000, height: 700 })).toEqual({ x: -30, y: -20, width: 280, height: 220 });
    expect(placedRect({ x: 10, y: 10, width: 400, height: 300 }, "maximized", { width: 800, height: 600 })).toEqual({ x: 0, y: 0, width: 800, height: 600 });
    expect(placedRect({ x: 10, y: 10, width: 400, height: 300 }, "maximized", { width: 800, height: 600 }, 38)).toEqual({ x: 0, y: 38, width: 800, height: 562 });
  });
  it("retains geometry and window identity when minimizing, focusing and restoring", () => {
    let state = shellReducer(initialShellState, { type: "open-window", window: { id: "one", title: "One", subtitle: "", location: "/app/one" } });
    const rect = { x: 120, y: 90, width: 500, height: 350 };
    state = shellReducer(state, { type: "set-window-rect", id: "one", rect });
    state = shellReducer(state, { type: "set-window-placement", id: "one", placement: "maximized" });
    state = shellReducer(state, { type: "minimize-all" });
    expect(state.windows[0]?.minimized).toBe(true);
    state = shellReducer(state, { type: "focus-window", id: "one" });
    expect(state.windows[0]?.rect).toEqual(rect);
    expect(state.windows[0]?.placement).toBe("maximized");
    state = shellReducer(state, { type: "set-window-placement", id: "one", placement: "floating" });
    expect(state.windows[0]?.rect).toEqual(rect);
    expect(shellReducer(state, { type: "focus-window", id: "missing" })).toBe(state);
  });
  it("selects desktop icons the marquee touches", () => {
    expect(rectsIntersect({ x: 0, y: 0, width: 10, height: 10 }, { x: 5, y: 5, width: 10, height: 10 })).toBe(true);
    expect(rectsIntersect({ x: 0, y: 0, width: 10, height: 10 }, { x: 10, y: 0, width: 10, height: 10 })).toBe(false);
    expect(rectsIntersect({ x: 0, y: 0, width: 10, height: 10 }, { x: 2, y: 2, width: 2, height: 2 })).toBe(true);
  });
});
