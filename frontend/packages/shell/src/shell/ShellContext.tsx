import { createContext, useContext, type Dispatch } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import type { ShellLiveSource } from "../live-updates";
import type { ShellAction, ShellState } from "../shell-state";
import type { ShellMode } from "../routing/paths";

export interface ShellContextValue {
  snapshot: ShellSnapshotV1;
  live: ShellLiveSource | undefined;
  state: ShellState;
  dispatch: Dispatch<ShellAction>;
  mode: ShellMode;
  setMode: (mode: ShellMode) => void;
  open: (path: string) => void;
}
export const ShellContext = createContext<ShellContextValue | null>(null);
export function useShell(): ShellContextValue {
  const context = useContext(ShellContext);
  if (!context) throw new Error("Shell context missing");
  return context;
}
