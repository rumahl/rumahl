import { useEffect, useMemo, useReducer } from "react";
import { useLocation, useNavigate } from "react-router";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import type { ShellLiveSource } from "../live-updates";
import { useAppCatalog } from "../apps/AppCatalog";
import { Navigation } from "../components/Navigation";
import { useI18n } from "../i18n";
import { initialShellState, shellReducer, type ShellWindow } from "../shell-state";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { localPath, type ShellMode } from "../routing/paths";
import { RouteBoundary } from "../routing/RouteBoundary";
import { ShellContext } from "./ShellContext";
import { Topbar } from "./Topbar";
import { CommandPalette } from "./CommandPalette";
import { DesktopWindows } from "./DesktopWindows";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { HomePage } from "../pages/HomePage";

export function ShellLayout({ snapshot, live }: { snapshot: ShellSnapshotV1; live: ShellLiveSource | undefined }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { t } = useI18n();
  const path = location.pathname + location.search;
  const preferences = useShellPreferences();
  const mode = preferences.mode;
  useEffect(() => {
    if (new URLSearchParams(location.search).has("mode")) void navigate(localPath(path + location.hash), { replace: true });
  }, [location.search, location.hash, path, navigate]);
  const route = describeRoute(path);
  const catalog = useAppCatalog();
  const installedTitle = catalog.apps.find((app) => app.id === route.appId)?.title;
  const routedWindow = useMemo<Omit<ShellWindow, "minimized">>(() => ({
    id: route.id, title: route.streamTitle ?? installedTitle ?? t(route.title), subtitle: t("appManager.subtitle"),
    location: path, ...(route.stream ? { streamId: route.id.slice(7) } : {})
  }), [installedTitle, route.id, route.title, route.streamTitle, route.stream, path, t]);
  const [state, dispatch] = useReducer(shellReducer, initialShellState, (initial) =>
    mode === "desktop" && route.presentation === "window" ? shellReducer(initial, { type: "open-window", window: routedWindow }) : initial);
  useEffect(() => {
    if (mode === "desktop" && route.presentation === "window") {
      dispatch({ type: "open-window", window: routedWindow });
    }
  }, [mode, routedWindow, route.presentation]);
  const open = (target: string) => { void navigate(localPath(target)); };
  const setMode = (next: ShellMode) => { preferences.save("device", next); };
  return <ShellContext value={{ snapshot, live, state, dispatch, mode, setMode, open }}>
    <div className="shell" data-shell-build={snapshot.shellBuildId} data-shell-mode={mode} data-preferences-revision={preferences.preferences?.revision}>
      <Navigation active={route.section} />
      <div className="shell__workspace">
        <Topbar />
        <div className={mode === "launcher" ? "launcher-workspace" : "desktop-workspace"}>
          <RouteBoundary location={path}>
            {mode === "desktop" && route.presentation === "window" ? <HomePage /> : <ShellRoutes />}
          </RouteBoundary>
        </div>
        {mode === "desktop" ? <DesktopWindows /> : null}
        <CommandPalette />
      </div>
    </div>
  </ShellContext>;
}
