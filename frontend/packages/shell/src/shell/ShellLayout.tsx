import { useEffect, useMemo, useReducer, useRef } from "react";
import { useLocation, useNavigate } from "react-router";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { useTheme } from "@rumahl/ui";
import type { ShellLiveSource } from "../live-updates";
import { useAppCatalog } from "../apps/AppCatalog";
import { rememberRecent } from "../apps/recents";
import { DesktopLayout } from "./desktop/DesktopLayout";
import { LauncherLayout } from "./launcher/LauncherLayout";
import { OsDock } from "./OsDock";
import { Taskbar } from "./Taskbar";
import { useI18n } from "../i18n";
import { initialShellState, shellReducer, type ShellWindow } from "../shell-state";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { localPath, type ShellMode } from "../routing/paths";
import { RouteBoundary } from "../routing/RouteBoundary";
import { ShellContext } from "./ShellContext";
import { MenuBar } from "./MenuBar";
import { CommandPalette } from "./CommandPalette";
import { DesktopWindows } from "./DesktopWindows";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useWorkspace } from "../preferences/Workspace";
import { HomePage } from "../pages/HomePage";

export function ShellLayout({ snapshot, live }: { snapshot: ShellSnapshotV1; live: ShellLiveSource | undefined }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { variants } = useTheme();
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
    route.presentation === "window" ? shellReducer(initial, { type: "open-window", window: routedWindow }) : initial);
  useEffect(() => {
    if (route.presentation === "window") {
      dispatch({ type: "open-window", window: routedWindow });
    }
  }, [mode, routedWindow, route.presentation]);
  const workspace = useWorkspace();
  const restored = useRef(-1);
  useEffect(() => {
    if (!workspace.ready || catalog.status === "loading" || restored.current === workspace.restore) return;
    restored.current = workspace.restore;
    if (workspace.restore === 0 && workspace.effective.windows.length === 0) return;
    const windows: ShellWindow[] = [];
    for (const saved of workspace.effective.windows) {
      const description = describeRoute(saved.location);
      if (description.presentation !== "window" || description.stream || windows.some(w => w.id === description.id)) continue;
      const title = catalog.apps.find(app => app.id === description.appId)?.title ?? t(description.title);
      windows.push({ ...saved, id: description.id, title, subtitle: t("appManager.subtitle") });
    }
    // An explicit deep link always wins over a stored layout.
    if (route.presentation === "window") {
      const existing = windows.find(w => w.id === routedWindow.id);
      const active = { ...existing, ...routedWindow, minimized: false };
      dispatch({ type: "restore-workspace", windows: [...windows.filter(w => w.id !== active.id), active] });
    } else dispatch({ type: "restore-workspace", windows });
  }, [workspace.ready, workspace.restore, workspace.effective, catalog.status, catalog.apps, route.presentation, routedWindow, t]);
  const open = (target: string) => { const next = localPath(target); rememberRecent(next); void navigate(next); };
  const setMode = (next: ShellMode) => { preferences.save("device", next); };
  return <ShellContext value={{ snapshot, live, state, dispatch, mode, setMode, open }}>
    <div className="shell" data-shell-build={snapshot.shellBuildId} data-shell-mode={mode} data-preferences-revision={preferences.preferences?.revision}>
      <div className="shell__workspace">
        <MenuBar />
        <div className="workspace-surfaces">
          {mode === "desktop" ? <DesktopLayout><RouteBoundary location={path}>{route.presentation === "window" ? <HomePage /> : <ShellRoutes />}</RouteBoundary></DesktopLayout>
            : <LauncherLayout><RouteBoundary location={path}>{route.presentation === "window" ? null : <ShellRoutes />}</RouteBoundary></LauncherLayout>}
          <DesktopWindows />
        </div>
        {variants.shellLayout === "taskbar" ? <Taskbar /> : <OsDock />}
        <CommandPalette />
      </div>
    </div>
  </ShellContext>;
}
