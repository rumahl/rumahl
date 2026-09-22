export type ShellSection = "home" | "apps" | "activity" | "settings";

export interface ShellWindow {
  id: string;
  title: string;
  subtitle: string;
  minimized: boolean;
}

export interface ShellState {
  section: ShellSection;
  windows: readonly ShellWindow[];
  focusedWindowId: string | null;
  commandPaletteOpen: boolean;
}

export type ShellAction =
  | { type: "navigate"; section: ShellSection }
  | { type: "open-window"; window: Omit<ShellWindow, "minimized"> }
  | { type: "close-window"; id: string }
  | { type: "toggle-minimize"; id: string }
  | { type: "focus-window"; id: string }
  | { type: "toggle-command-palette" };

export const initialShellState: ShellState = {
  section: "home",
  windows: [],
  focusedWindowId: null,
  commandPaletteOpen: false
};

export function shellReducer(state: ShellState, action: ShellAction): ShellState {
  switch (action.type) {
    case "navigate":
      return { ...state, section: action.section };
    case "open-window": {
      const existing = state.windows.find((window) => window.id === action.window.id);
      const windows = existing
        ? state.windows.map((window) =>
            window.id === action.window.id ? { ...window, minimized: false } : window
          )
        : [...state.windows, { ...action.window, minimized: false }];
      return { ...state, windows, focusedWindowId: action.window.id };
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
    case "focus-window":
      return {
        ...state,
        windows: state.windows.map((window) =>
          window.id === action.id ? { ...window, minimized: false } : window
        ),
        focusedWindowId: action.id
      };
    case "toggle-command-palette":
      return { ...state, commandPaletteOpen: !state.commandPaletteOpen };
  }
}
