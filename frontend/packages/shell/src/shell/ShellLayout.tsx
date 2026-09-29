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

function WallpaperVideo({ src }: { src: string }) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    video.muted = true;
    const played = video.play();
    if (played) void played.catch(() => undefined);
  }, [src]);
  return <video ref={ref} className="wallpaper-video" src={src} autoPlay muted loop playsInline />;
}
import { resolveWallpaper } from "../wallpapers";
import { configureGlass, svgRefractionSupported } from "../glass-engine/useGlassEngine";
import { useTestMedia } from "./test-media";

export function ShellLayout({ snapshot, live }: { snapshot: ShellSnapshotV1; live: ShellLiveSource | undefined }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, variants, tokens } = useTheme();
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
  const workspace = useWorkspace();
  // Restores the device workspace windows. Available synchronously from the
  // snapshot so windows are server-rendered, not added after a client fetch.
  const restoredWindows = () => {
    const windows: ShellWindow[] = [];
    for (const saved of workspace.effective.windows) {
      const description = describeRoute(saved.location);
      if (description.presentation !== "window" || description.stream || windows.some(w => w.id === description.id)) continue;
      const title = catalog.apps.find(app => app.id === description.appId)?.title ?? t(description.title);
      windows.push({ ...saved, id: description.id, title, subtitle: t("appManager.subtitle") });
    }
    return windows;
  };
  const [state, dispatch] = useReducer(shellReducer, initialShellState, (initial) => {
    const seeded = restoredWindows();
    const next = seeded.length ? shellReducer(initial, { type: "restore-workspace", windows: seeded }) : initial;
    return route.presentation === "window" ? shellReducer(next, { type: "open-window", window: routedWindow }) : next;
  });
  useEffect(() => {
    if (route.presentation === "window") {
      dispatch({ type: "open-window", window: routedWindow });
    }
  }, [mode, routedWindow, route.presentation]);
  const restored = useRef(-1);
  useEffect(() => {
    if (!workspace.ready || catalog.status === "loading" || restored.current === workspace.restore) return;
    restored.current = workspace.restore;
    if (workspace.restore === 0 && workspace.effective.windows.length === 0) return;
    const windows = restoredWindows();
    // An explicit deep link always wins over a stored layout.
    if (route.presentation === "window") {
      const existing = windows.find(w => w.id === routedWindow.id);
      const active = { ...existing, ...routedWindow, minimized: false };
      dispatch({ type: "restore-workspace", windows: [...windows.filter(w => w.id !== active.id), active] });
    } else dispatch({ type: "restore-workspace", windows });
  }, [workspace.ready, workspace.restore, workspace.effective, catalog.status, catalog.apps, route.presentation, routedWindow, t]);
  const open = (target: string) => { const next = localPath(target); rememberRecent(next); void navigate(next); };
  const setMode = (next: ShellMode) => { preferences.save("device", next); };
  const glassEngine = workspace.effective.appearance.glassEngine ?? "canvas";
  const material = Number(tokens["material.opacity"]) >= 1 || Number(tokens["material.morphism"]) <= 0 ? "solid" : "translucent";
  const scheme = workspace.effective.appearance.mode ?? "light";
  const wallpaper = resolveWallpaper(tokens["texture.wallpaper"]);
  const wallpaperUrl = /^url\((.*)\)$/.exec(wallpaper)?.[1]?.replace(/^["']|["']$/g, "") ?? null;
  const media = useTestMedia();
  const showVideo = media.video !== null;
  const mediaSrc = showVideo ? media.video : media.image ?? wallpaperUrl;
  useEffect(() => {
    if (typeof document === "undefined") return;
    // The reference stylesheet keys its light theme off `body.light`.
    document.body.classList.toggle("light", scheme === "light");
  }, [scheme]);
  const appearance = workspace.effective.appearance;
  const glassEnabled = appearance.glassEnabled !== false;
  // SVG (`backdrop-filter: url()`) cannot sample a composited video layer, so a
  // video wallpaper uses WebGL, which reads the video texture directly.
  const glassBackend = appearance.glassBackend ?? "auto";
  const glassQuality = appearance.glassQuality ?? "auto";
  // An animated (promoted) wallpaper is excluded from the SVG backdrop, so SVG
  // switches the wallpaper to a still layer.
  const svgActive = glassEnabled && (glassBackend === "svg" || (glassBackend === "auto" && svgRefractionSupported()));
  useEffect(() => {
    configureGlass({ enabled: glassEnabled, backend: glassBackend, quality: glassQuality });
  }, [glassEnabled, glassBackend, glassQuality]);
  return <ShellContext value={{ snapshot, live, state, dispatch, mode, setMode, open }}>
    <div className={`scene shell${showVideo ? "" : " image-mode"}${appearance.wallpaperMotion === false || svgActive ? " no-motion" : ""}`} data-theme={theme.id} data-material={material} data-glass={glassEngine} data-scheme={scheme} data-shell-build={snapshot.shellBuildId} data-shell-mode={mode} data-preferences-revision={preferences.preferences?.revision}>
      <div className="wallpaper media-wall" aria-hidden="true">
        {showVideo
          ? media.video ? <WallpaperVideo key={media.video} src={media.video} />
            : null
          : mediaSrc ? <img key={mediaSrc} className="wallpaper-image" src={mediaSrc} alt="" />
            : null}
      </div>
      <div className="shell__workspace">
        <MenuBar />
        <div className="workspace-surfaces">
          {mode === "desktop" ? <DesktopLayout><RouteBoundary location={path}>{route.presentation === "window" ? <HomePage /> : <ShellRoutes />}</RouteBoundary></DesktopLayout>
            : <LauncherLayout><RouteBoundary location={path}>{route.presentation === "window" ? null : <ShellRoutes />}</RouteBoundary></LauncherLayout>}
          <DesktopWindows />
        </div>
        <CommandPalette />
      </div>
      {variants.shellLayout === "taskbar" ? <Taskbar /> : <OsDock />}
    </div>
  </ShellContext>;
}
