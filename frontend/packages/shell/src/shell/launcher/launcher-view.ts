import { useCallback } from "react";
import { useWorkspace } from "../../preferences/Workspace";

export type LauncherView = "grid" | "deck" | "canvas";
const VIEWS: readonly LauncherView[] = ["grid", "deck", "canvas"];

/**
 * Launcher layout, stored in the device-scoped workspace so it is rendered on
 * the first paint. When unset, the theme's default view applies.
 */
export function useLauncherView(defaultView: LauncherView = "grid"): [LauncherView, (view: LauncherView) => void] {
  const workspace = useWorkspace();
  const stored = workspace.effective.launcherView;
  const view = stored && (VIEWS as readonly string[]).includes(stored) ? stored : defaultView;
  const update = useCallback((next: LauncherView) => {
    void workspace.save("device", { ...workspace.effective, launcherView: next });
  }, [workspace]);
  return [view, update];
}
