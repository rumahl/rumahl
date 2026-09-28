import { useCallback, useEffect, useState } from "react";

export type LauncherView = "grid" | "deck" | "canvas";
const KEY = "rumahl.launcher.view.v1";
const VIEWS: readonly LauncherView[] = ["grid", "deck", "canvas"];

/** Device-local launcher layout. Starts on the deterministic default for SSR. */
export function useLauncherView(): [LauncherView, (view: LauncherView) => void] {
  const [view, setView] = useState<LauncherView>("grid");
  useEffect(() => {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw && (VIEWS as readonly string[]).includes(raw)) setView(raw as LauncherView);
    } catch {
      // Restricted storage keeps the default view for this session.
    }
  }, []);
  const update = useCallback((next: LauncherView) => {
    setView(next);
    try { localStorage.setItem(KEY, next); } catch { /* session-only view */ }
  }, []);
  return [view, update];
}
