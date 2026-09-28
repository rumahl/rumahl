import type { WindowRect, WindowPlacement } from "./shell/desktop/geometry";
export type ShellSection = "home" | "apps" | "activity" | "settings";

export interface ShellWindow {
  id: string;
  title: string;
  subtitle: string;
  streamId?: string;
  location?: string;
  minimized: boolean;
  rect?: WindowRect;
  placement?: WindowPlacement;
}

export interface ShellState {
  windows: readonly ShellWindow[];
  focusedWindowId: string | null;
  commandPaletteOpen: boolean;
}

export type ShellAction =
  | { type: "restore-workspace"; windows: ShellWindow[] }
  | { type: "open-window"; window: Omit<ShellWindow, "minimized"> }
  | { type: "close-window"; id: string }
  | { type: "toggle-minimize"; id: string }
  | { type: "focus-window"; id: string }
  | { type: "set-window-rect"; id: string; rect: WindowRect }
  | { type: "set-window-placement"; id: string; placement: WindowPlacement }
  | { type: "minimize-all" }
  | { type: "toggle-command-palette" };

export const initialShellState: ShellState = {
  windows: [],
  focusedWindowId: null,
  commandPaletteOpen: false
};

export function shellReducer(state: ShellState, action: ShellAction): ShellState {
  switch (action.type) {
    case "restore-workspace": return { ...state, windows: action.windows, focusedWindowId: action.windows.filter(w => !w.minimized).at(-1)?.id ?? null };
    case "open-window": {
      const existing = state.windows.find((window) => window.id === action.window.id);
      const windows = existing
        ? state.windows.map((window) =>
            window.id === action.window.id ? { ...window, ...action.window, minimized: false } : window
          )
        : [...state.windows, { ...action.window, rect: { x: 36 + state.windows.length % 6 * 28, y: 24 + state.windows.length % 6 * 28, width: 760, height: 540 }, placement: "floating" as const, minimized: false }];
      return { ...state, windows: [...windows.filter((item) => item.id !== action.window.id), windows.find((item) => item.id === action.window.id)!], focusedWindowId: action.window.id };
    }
    case "close-window": {
      const windows = state.windows.filter((window) => window.id !== action.id);
      return {
        ...state,
        windows,
        focusedWindowId:
          state.focusedWindowId === action.id ? (windows.at(-1)?.id ?? null) : state.focusedWindowId
      };
    }
    case "toggle-minimize": {
      const windows = state.windows.map((window) =>
        window.id === action.id ? { ...window, minimized: !window.minimized } : window
      );
      const minimized = windows.find((window) => window.id === action.id)?.minimized ?? false;
      return {
        ...state,
        windows,
        focusedWindowId: minimized ? null : action.id
      };
    }
    case "focus-window": {
      const window = state.windows.find((item) => item.id === action.id);
      if (!window) return state;
      return { ...state, windows: [...state.windows.filter((item) => item.id !== action.id), { ...window, minimized: false }], focusedWindowId: action.id };
    }
    case "set-window-rect":
      if (Object.values(action.rect).some((value) => !Number.isFinite(value))) return state;
      return { ...state, windows: state.windows.map((item) => item.id === action.id ? { ...item, rect: action.rect } : item) };
    case "set-window-placement":
      return { ...state, windows: state.windows.map((item) => item.id === action.id ? { ...item, placement: action.placement } : item) };
    case "minimize-all":
      return { ...state, windows: state.windows.map((item) => ({ ...item, minimized: true })), focusedWindowId: null };
    case "toggle-command-palette":
      return { ...state, commandPaletteOpen: !state.commandPaletteOpen };
  }
}
