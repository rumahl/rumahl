import { describe, expect, test } from "vitest";
import { initialShellState, shellReducer, type ShellState } from "./shell-state";

function twoWindows(): ShellState {
  return shellReducer(
    shellReducer(initialShellState, { type: "open-window", window: { id: "app:a", title: "A", subtitle: "" } }),
    { type: "open-window", window: { id: "app:b", title: "B", subtitle: "" } }
  );
}

describe("shellReducer open-window", () => {
  test("keeps the window order stable when focusing an existing window", () => {
    const next = shellReducer(twoWindows(), {
      type: "open-window",
      window: { id: "app:a", title: "A", subtitle: "", location: "/app/a" }
    });
    expect(next.windows.map((window) => window.id)).toEqual(["app:a", "app:b"]);
    expect(next.focusedWindowId).toBe("app:a");
    expect(next.windows[0]).toMatchObject({ location: "/app/a", minimized: false });
  });

  test("appends a new window and refuses to reorder existing ones", () => {
    const next = shellReducer(twoWindows(), { type: "open-window", window: { id: "app:c", title: "C", subtitle: "" } });
    expect(next.windows.map((window) => window.id)).toEqual(["app:a", "app:b", "app:c"]);
    expect(next.focusedWindowId).toBe("app:c");
  });

  test("toggles an exclusive fullscreen window without losing its geometry", () => {
    let state = shellReducer(initialShellState, { type: "open-window", window: { id: "one", title: "One", subtitle: "", location: "/app/one" } });
    const rect = { x: 120, y: 90, width: 500, height: 350 };
    state = shellReducer(state, { type: "set-window-rect", id: "one", rect });
    state = shellReducer(state, { type: "set-window-fullscreen", id: "one", fullscreen: true });
    expect(state.windows[0]?.fullscreen).toBe(true);
    expect(state.windows[0]?.minimized).toBe(false);
    expect(state.focusedWindowId).toBe("one");
    state = shellReducer(state, { type: "set-window-fullscreen", id: "one", fullscreen: false });
    expect(state.windows[0]?.fullscreen).toBe(false);
    expect(state.windows[0]?.rect).toEqual(rect);
  });
});

describe("app switcher overlay", () => {
  test("toggles and closes independently of the window list", () => {
    let state = shellReducer(twoWindows(), { type: "toggle-app-switcher" });
    expect(state.appSwitcherOpen).toBe(true);
    expect(state.windows).toHaveLength(2);
    state = shellReducer(state, { type: "toggle-app-switcher" });
    expect(state.appSwitcherOpen).toBe(false);
    state = shellReducer(shellReducer(state, { type: "toggle-app-switcher" }), { type: "close-app-switcher" });
    expect(state.appSwitcherOpen).toBe(false);
  });
});
