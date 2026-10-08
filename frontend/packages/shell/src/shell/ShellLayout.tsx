import { isInternalTarget, shellLocation } from "../routing/internal";
import { readInternalWindows, writeInternalWindows } from "./internal-windows";
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { useTheme } from "@rumahl/ui";
import type { ShellLiveSource } from "../live-updates";
import { useAppCatalog } from "../apps/AppCatalog";
import { rememberRecent } from "../apps/recents";
import { DesktopLayout } from "./desktop/DesktopLayout";
import { LauncherLayout } from "./launcher/LauncherLayout";
import { OsDock } from "./OsDock";
import { SlotRegion } from "./SlotRegion";
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
import { useOsMode } from "../preferences/OsMode";
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
import { configureGlass, svgRefractionSupported, subscribeGlassMetrics } from "../glass-engine/useGlassEngine";
import { useTestMedia } from "./test-media";
import { isMobile } from "./device";
import { isDebug, setDebug, useDebug } from "./debug";
import { useSystemMode } from "./system-scheme";
import { getToasts, installToastApi, setToastAnimations, showToast, ToastStack } from "./toasts";

export function ShellLayout({ snapshot, live }: { snapshot: ShellSnapshotV1; live: ShellLiveSource | undefined }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, variants, tokens } = useTheme();
  const { t } = useI18n();
  const path = shellLocation(location.pathname + location.search + location.hash);
  const preferences = useShellPreferences();
  const osMode = useOsMode();
  const mode = preferences.mode;
  useEffect(() => {
    if (new URLSearchParams(location.search).has("mode")) void navigate(localPath(location.pathname + location.search + location.hash), { replace: true });
  }, [location.search, location.hash, path, navigate]);
  const route = describeRoute(path);
  // In launcher mode an opened app replaces the home surface; the top bar then
  // slides away and the window keeps the space the bar occupied.
  const launcherWindow = mode === "launcher" && route.presentation === "window";
  const catalog = useAppCatalog();
  const installedTitle = catalog.apps.find((app) => app.id === route.appId)?.title;
  const routedWindow = useMemo<Omit<ShellWindow, "minimized">>(() => ({
    id: route.id, title: route.streamTitle ?? installedTitle ?? t(route.title), subtitle: t("appManager.subtitle"),
    location: path, ...(route.stream ? { streamId: route.id.slice(7) } : {})
  }), [installedTitle, route.id, route.title, route.streamTitle, route.stream, path, t]);
  const workspace = useWorkspace();
  // Restores the device workspace windows. Available synchronously from the
  // snapshot so windows are server-rendered, not added after a client fetch.
  const restoredWindows = (includeInternal = true) => {
    const windows: ShellWindow[] = [];
    for (const saved of workspace.effective.windows) {
      const description = describeRoute(saved.location);
      if (description.presentation !== "window" || description.stream || windows.some(w => w.id === description.id)) continue;
      const title = catalog.apps.find(app => app.id === description.appId)?.title ?? t(description.title);
      windows.push({ ...saved, id: description.id, title, subtitle: t("appManager.subtitle") });
    }
    for (const saved of includeInternal ? readInternalWindows() : []) {
      const description = describeRoute(saved.location!);
      windows.push({ ...saved, id: description.id, title: t(description.title), subtitle: t("appManager.subtitle") });
    }
    return windows;
  };
  const [state, dispatch] = useReducer(shellReducer, initialShellState, (initial) => {
    const seeded = restoredWindows(false);
    const next = seeded.length ? shellReducer(initial, { type: "restore-workspace", windows: seeded }) : initial;
    return route.presentation === "window" ? shellReducer(next, { type: "open-window", window: routedWindow }) : next;
  });
  useEffect(() => {
    if (route.presentation === "window") {
      dispatch({ type: "open-window", window: routedWindow });
    }
  }, [mode, routedWindow, route.presentation]);
  const [internalReady, setInternalReady] = useState(false);
  useEffect(() => {
    // Internal tools (the design lab) are gated by the OS mode.
    if (!osMode.policy.browseSystemFiles) {
      setInternalReady(false);
      dispatch({ type: "restore-workspace", windows: state.windows.filter(w => !w.location || !isInternalTarget(shellLocation(w.location))) });
      return;
    }
    setInternalReady(true);
    const saved = readInternalWindows().map(w => ({ ...w, id: describeRoute(w.location!).id, title: t(describeRoute(w.location!).title), subtitle: t("appManager.subtitle") }));
    if (saved.length) dispatch({ type: "restore-workspace", windows: [
      ...saved.filter(w => !state.windows.some(current => current.location === w.location)),
      ...state.windows.map(current => {
        const previous = saved.find(w => w.location === current.location);
        return previous ? { ...current, ...previous, minimized: current.location === path ? false : previous.minimized } : current;
      })
    ] });
  }, [osMode.policy.browseSystemFiles]);
  useEffect(() => { if (internalReady) writeInternalWindows(state.windows); }, [state.windows, internalReady]);
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
  const open = (target: string) => { const next = localPath(target); if (!isInternalTarget(shellLocation(target))) rememberRecent(next); void navigate(next); };
  const setMode = (next: ShellMode) => {
    preferences.save("device", next);
    if (next === "desktop" && isMobile()) showToast({ message: t("perf.desktopMobile"), variant: "warning", duration: 6000 });
  };
  const glassEngine = workspace.effective.appearance.glassEngine ?? "canvas";
  const systemMode = useSystemMode();
  const scheme = workspace.effective.appearance.mode ?? systemMode;
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
  const performanceMode = appearance.performanceMode === true;
  const animations = appearance.animations !== false && !performanceMode;
  // Transparency is the base layer; glass only works with transparency on.
  const transparencyOff = performanceMode || Number(tokens["material.opacity"]) >= 1 || Number(tokens["material.morphism"]) <= 0;
  const glassEnabled = appearance.glassEnabled !== false && !transparencyOff;
  const glassReduced = appearance.glassReduced === true || performanceMode;
  const setGlassReduced = useCallback((value: boolean) => {
    void workspace.save("device", { ...workspace.effective, appearance: { ...workspace.effective.appearance, glassReduced: value } });
  }, [workspace]);
  const setTransparencyOff = useCallback(() => {
    void workspace.save("device", { ...workspace.effective, appearance: { ...workspace.effective.appearance, tokens: { ...workspace.effective.appearance.tokens, "material.opacity": "1", "material.blur": "0px", "material.saturation": "1", "material.morphism": "0" } } });
  }, [workspace]);
  // SVG (`backdrop-filter: url()`) cannot sample a composited video layer, so a
  // video wallpaper uses WebGL, which reads the video texture directly.
  const glassBackend = appearance.glassBackend ?? "auto";
  const glassQuality = appearance.glassQuality ?? "auto";
  // Automatically disabled at load when the browser is too weak for glass, and
  // offered via a toast when performance drops while running.
  const [glassOff, setGlassOff] = useState(false);
  const [inactive, setInactive] = useState(false);
  const debug = useDebug();
  const mobileChecked = useRef(false);
  const stressWindows = useRef(0);
  const effectiveGlass = glassEnabled && (!glassOff || appearance.glassEnabled === true) && !glassReduced;
  // A slow renderer may fall back to CSS, but must not override transparency.
  const material = transparencyOff ? "solid" : "translucent";
  // An animated (promoted) wallpaper is excluded from the SVG backdrop, so SVG
  // switches the wallpaper to a still layer.
  // Resolve the SVG backdrop after mount so the server and the first client
  // render agree on the scene classes (hydration-safe).
  const [svgActive, setSvgActive] = useState(false);
  useEffect(() => {
    setSvgActive(glassEnabled && (glassBackend === "svg" || (glassBackend === "auto" && svgRefractionSupported())));
  }, [glassEnabled, glassBackend]);
  const windowMaximized = state.windows.some((item) => !item.minimized && (item.placement ?? "floating") !== "floating");
  useEffect(() => {
    configureGlass({ enabled: effectiveGlass, backend: glassBackend, quality: glassQuality });
  }, [effectiveGlass, glassBackend, glassQuality]);
  useEffect(() => {
    setToastAnimations(animations);
    if (typeof document !== "undefined") document.body.classList.toggle("animations-off", !animations);
  }, [animations]);
  // On phones the launcher is the sensible default; the desktop mode is not
  // fully usable there (we warn if the user switches to it).
  useEffect(() => {
    if (mobileChecked.current) return;
    mobileChecked.current = true;
    if (isMobile() && preferences.mode === "desktop") preferences.save("device", "launcher");
  }, [preferences]);
  // One short startup benchmark: if the browser cannot keep up, glass is off
  // from the first paint instead of degrading later.
  useEffect(() => {
    if (typeof window === "undefined" || typeof requestAnimationFrame !== "function") return;
    let raf = 0, frames = 0, dropped = 0;
    let last = performance.now();
    const start = last;
    const tick = (now: number) => {
      const dt = now - last; last = now;
      if (dt > 0 && dt < 250) { frames++; if (dt > (1000 / 50) * 1.55) dropped++; }
      if (now - start < 1200) raf = requestAnimationFrame(tick);
      else if (frames > 10 && dropped / frames > 0.35) setGlassOff(true);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  // While running, only *suggest* reducing glass (the drop may be another app).
  useEffect(() => subscribeGlassMetrics((metrics) => {
    // Automatic, escalating response to *sustained* slowness (ignores momentary
    // load spikes; needs 3 consecutive slow windows). First drop glass only;
    // if it stays slow, drop transparency entirely.
    if (performanceMode || inactive || glassOff || appearance.glassEnabled === true) return;
    if (transparencyOff && !glassEnabled) return;
    if (metrics.dropped <= 0.3) { stressWindows.current = 0; return; }
    stressWindows.current += 1;
    if (stressWindows.current < 3) return;
    stressWindows.current = 0;
    if (glassEnabled && !glassReduced) {
      setGlassReduced(true);
      showToast({ message: t("perf.glassOff"), variant: "info", duration: null, buttons: [{ label: t("perf.undo"), variant: "primary", onClick: () => setGlassReduced(false) }, { label: t("perf.dismiss") }] });
    } else if (!transparencyOff) {
      setTransparencyOff();
      showToast({ message: t("perf.transparencyOff"), variant: "info", duration: 6000 });
    }
  }), [performanceMode, inactive, glassOff, appearance.glassEnabled, transparencyOff, glassEnabled, glassReduced, setGlassReduced, setTransparencyOff, t]);
  // Debug mode + console API (`window.__rumahlDebug`), enable-able only from devtools.
  useEffect(() => {
    if (typeof window === "undefined") return;
    installToastApi();
    const api = {
      enable: () => setDebug(true),
      disable: () => setDebug(false),
      toggle: () => setDebug(!isDebug()),
      isEnabled: () => isDebug(),
      toast: (message?: string) => showToast({ message: message ?? t("perf.toastMessage"), variant: "info", duration: 4000 }),
      toastCount: () => getToasts().length,
      demo: () => {
        showToast({ title: "Success", message: "App installed.", variant: "success", duration: 5000 });
        showToast({ title: "Warning", message: "Low disk space.", variant: "warning", duration: null, buttons: [{ label: "Manage", variant: "primary", onClick: () => undefined }, { label: "Later" }] });
        showToast({ message: "This one stays until you close it.", variant: "info", duration: null });
      },
      state: () => ({
        renderer: document.documentElement.dataset.glassRenderer,
        backend: glassBackend,
        quality: glassQuality,
        glassOff,
        inactive,
        debug: isDebug(),
        theme: theme.id
      })
    };
    (window as unknown as { __rumahlDebug?: typeof api }).__rumahlDebug = api;
  }, [glassBackend, glassQuality, glassOff, inactive, theme.id, t]);
  // Pause animations/clock while the tab or browser window is not visible
  // (imperceptible: everything resumes exactly where it left off).
  useEffect(() => {
    if (typeof document === "undefined") return;
    const update = () => setInactive(document.hidden || (typeof document.hasFocus === "function" && !document.hasFocus()));
    update();
    document.addEventListener("visibilitychange", update);
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    return () => { document.removeEventListener("visibilitychange", update); window.removeEventListener("focus", update); window.removeEventListener("blur", update); };
  }, []);
  // Theme stylesheet via constructable stylesheets (not blocked by `style-src`).
  useEffect(() => {
    if (!theme.css || typeof document === "undefined" || typeof CSSStyleSheet === "undefined" || !("adoptedStyleSheets" in document)) return;
    const sheet = new CSSStyleSheet();
    try { sheet.replaceSync(theme.css); } catch { return; }
    const previous = document.adoptedStyleSheets;
    document.adoptedStyleSheets = [...previous, sheet];
    return () => { document.adoptedStyleSheets = previous; };
  }, [theme.css]);
  return <ShellContext value={{ snapshot, live, state, dispatch, mode, setMode, open }}>
    <div className={`scene shell${showVideo ? "" : " image-mode"}${appearance.wallpaperMotion === false || svgActive || performanceMode ? " no-motion" : ""}`} data-theme={theme.id} data-material={material} data-glass={glassEngine} data-scheme={scheme} data-maximized={windowMaximized ? "true" : undefined} data-launcher={launcherWindow ? "app" : undefined} data-inactive={inactive ? "true" : undefined} data-debug={debug ? "true" : undefined} data-animations={animations ? undefined : "off"} data-shell-build={snapshot.shellBuildId} data-shell-mode={mode} data-os-mode={osMode.mode} data-preferences-revision={preferences.preferences?.revision}>
      <div className="wallpaper media-wall" aria-hidden="true">
        {showVideo
          ? media.video ? <WallpaperVideo key={media.video} src={media.video} />
            : null
          : mediaSrc ? <img key={mediaSrc} className="wallpaper-image" src={mediaSrc} alt="" />
            : null}
      </div>
      {debug ? <div className="debug-badge" aria-hidden="true">debug · {glassBackend} · {glassQuality} · {glassOff ? "glass off" : "glass on"}</div> : null}
      <ToastStack />
      <div className="shell__workspace">
        {theme.slots?.menubar ? <SlotRegion name="menubar" /> : <MenuBar />}
        <div className="workspace-surfaces">
          {mode === "desktop" ? <DesktopLayout><RouteBoundary location={path}>{route.presentation === "window" ? <HomePage /> : <ShellRoutes />}</RouteBoundary></DesktopLayout>
            : <LauncherLayout><RouteBoundary location={path}>{route.presentation === "window" ? null : <ShellRoutes />}</RouteBoundary></LauncherLayout>}
          <DesktopWindows />
        </div>
        <CommandPalette />
      </div>
      {theme.slots?.dock ? <SlotRegion name="dock" /> : variants.shellLayout === "taskbar" ? <Taskbar /> : <OsDock />}
    </div>
  </ShellContext>;
}
