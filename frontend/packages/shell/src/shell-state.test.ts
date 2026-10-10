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
});
