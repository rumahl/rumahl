import { useCallback, useEffect, useState } from "react";

export type LauncherView = "grid" | "deck" | "canvas";
const KEY = "rumahl.launcher.view.v1";
const VIEWS: readonly LauncherView[] = ["grid", "deck", "canvas"];

/**
 * Device-local launcher layout. The theme supplies the default view
 * (`springboard` → grid, `drawer` → deck); a stored user choice wins.
 */
export function useLauncherView(defaultView: LauncherView = "grid"): [LauncherView, (view: LauncherView) => void] {
  const [view, setView] = useState<LauncherView>(defaultView);
  useEffect(() => {
    try {
      const raw = localStorage.getItem(KEY);
      setView(raw && (VIEWS as readonly string[]).includes(raw) ? raw as LauncherView : defaultView);
    } catch {
      setView(defaultView);
    }
  }, [defaultView]);
  const update = useCallback((next: LauncherView) => {
    setView(next);
    try { localStorage.setItem(KEY, next); } catch { /* session-only view */ }
  }, []);
  return [view, update];
}
