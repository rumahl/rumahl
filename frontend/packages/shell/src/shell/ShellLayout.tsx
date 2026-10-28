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
import { EmergencyShell } from "./EmergencyShell";
import { KeyPalette } from "./KeyPalette";
import { installShortcuts, useKeyPaletteOpen } from "./keyboard";
import { useEmergencyOpen } from "./emergency";
import { reportActivity } from "../apps/recovery-client";
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
import { useGlassHosts } from "../glass-engine/useGlassHosts";
import { useTestMedia } from "./test-media";
import { mobileDesktopOptIn, resolveShellMode, setMobileDesktopOptIn, useDeviceViewport } from "./device";
import { MobileOrientationHint } from "./MobileOrientationHint";
import { FrontendUpdateNotice } from "./FrontendUpdateNotice";
import { isDebug, setDebug, useDebug } from "./debug";
import { useSystemMode } from "./system-scheme";
import { getToasts, installToastApi, setToastAnimations, showToast, ToastStack } from "./toasts";

export function ShellLayout({ snapshot, live }: { snapshot: ShellSnapshotV1; live: ShellLiveSource | undefined }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, variants, tokens } = useTheme();
  // Upgrade @rumahl/ui GlassSurface hosts (dock, floating surfaces) to the
  // engine's real refraction glass. CSS in @rumahl/ui/glass.css is the fallback.
  useGlassHosts();
  const { t } = useI18n();
  const path = shellLocation(location.pathname + location.search + location.hash);
  const preferences = useShellPreferences();
  const osMode = useOsMode();
  const viewport = useDeviceViewport();
  const [desktopOptIn, setDesktopOptIn] = useState(false);
  const [desktopHint, setDesktopHint] = useState(false);
  const [hintDismissed, setHintDismissed] = useState(false);
  useEffect(() => { setDesktopOptIn(mobileDesktopOptIn()); }, []);
  // Phones always start in the launcher. The desktop is only reachable in
  // landscape *and* after the user opted in on this browser.
  const mode: ShellMode = resolveShellMode(preferences.mode, viewport, desktopOptIn);
  useEffect(() => {
    if (viewport.landscape) { setDesktopHint(false); setHintDismissed(false); return; }
    if (!viewport.mobile) { setDesktopHint(false); return; }
    if (desktopOptIn && preferences.mode === "desktop" && !hintDismissed) setDesktopHint(true);
  }, [viewport.mobile, viewport.landscape, desktopOptIn, preferences.mode, hintDismissed]);
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
    id: route.id, title: route.streamTitle ?? installedTitle ?? t(route.title), subtitle: "",
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
      windows.push({ ...saved, id: description.id, title, subtitle: "" });
    }
    for (const saved of includeInternal ? readInternalWindows() : []) {
      const description = describeRoute(saved.location!);
      windows.push({ ...saved, id: description.id, title: t(description.title), subtitle: "" });
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
    const saved = readInternalWindows().map(w => ({ ...w, id: describeRoute(w.location!).id, title: t(describeRoute(w.location!).title), subtitle: "" }));
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
    if (viewport.mobile) {
      setDesktopOptIn(next === "desktop");
      setMobileDesktopOptIn(next === "desktop");
    }
    if (next === "desktop" && viewport.mobile && !viewport.landscape) {
      // Keep the desktop preference: rotating to landscape opens it, while the
      // portrait shell stays on the launcher and the hint explains why.
      setHintDismissed(false);
      setDesktopHint(true);
    } else if (next === "launcher") {
      setDesktopHint(false);
    }
    preferences.save("device", next);
  };
  const glassEngine = workspace.effective.appearance.glassEngine ?? "canvas";
  const ultra = workspace.effective.appearance.ultraPerformance === true;
  const systemMode = useSystemMode();
  const scheme = workspace.effective.appearance.mode ?? systemMode;
  const wallpaper = resolveWallpaper(tokens["texture.wallpaper"]);
  const wallpaperUrl = /^url\((.*)\)$/.exec(wallpaper)?.[1]?.replace(/^["']|["']$/g, "") ?? null;
  const media = useTestMedia();
  const showVideo = media.video !== null && !ultra;
  const mediaSrc = ultra ? null : showVideo ? media.video : media.image ?? wallpaperUrl;
  useEffect(() => {
    if (typeof document === "undefined") return;
    // The reference stylesheet keys its light theme off `body.light`.
    document.body.classList.toggle("light", scheme === "light");
  }, [scheme]);
  const appearance = workspace.effective.appearance;
  const performanceMode = appearance.performanceMode === true || ultra;
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
  // The shell itself is always the normal Glass design. Floating is an element
  // state applied to individual elements that hover over a background (app
  // icons in the launcher, floating buttons), not a mode.
  const material = transparencyOff ? "solid" : "translucent";
  // Material of the rumahl/ui app input components, chosen by the theme. It is
  // independent of the shell material above: "solid" renders them flat.
  const appInputs = tokens["app.inputs"] === "solid" ? "solid" : "glass";
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
  // The launcher is the mobile default. A desktop preference only survives on
  // mobile after an explicit opt-in from the landscape view.
  useEffect(() => {
    if (mobileChecked.current || !viewport.mobile || !preferences.ready) return;
    mobileChecked.current = true;
    if (preferences.mode === "desktop" && !mobileDesktopOptIn()) preferences.save("device", "launcher");
  }, [viewport.mobile, preferences.ready, preferences.mode, preferences]);
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
  // On very weak devices, *suggest* the ultra performance mode once. It is never
  // enabled automatically, and is not shown again after being handled.
  useEffect(() => {
    if (typeof navigator === "undefined" || typeof localStorage === "undefined") return;
    if (ultra || localStorage.getItem("rumahl.ultra.prompt")) return;
    const cores = navigator.hardwareConcurrency || 8;
    const memory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory || 8;
    if (cores > 2 && memory > 2) return;
    localStorage.setItem("rumahl.ultra.prompt", "1");
    showToast({
      message: t("perf.ultraSuggest"),
      variant: "info",
      duration: null,
      buttons: [
        { label: t("perf.ultraEnable"), variant: "primary", onClick: () => void workspace.save("device", { ...workspace.effective, appearance: { ...workspace.effective.appearance, ultraPerformance: true } }) },
        { label: t("perf.dismiss") }
      ]
    });
  }, [ultra, t, workspace]);
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
  const emergencyOpen = useEmergencyOpen();
  const paletteOpen = useKeyPaletteOpen();
  // Hidden keyboard commands (leader `r`); the emergency chord goes to /recovery.
  useEffect(() => installShortcuts(), []);
  // Report shell activity (navigation, mode) into the central audit log.
  useEffect(() => { if (live) reportActivity(live.request, `navigate ${path}`); }, [path, live]);
  useEffect(() => { if (live) reportActivity(live.request, `mode ${mode}`); }, [mode, live]);
  // Mirror the base on <html> so portals (menus, context menus) can style themselves.
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.themeBase = theme.base ?? "rumahl";
    root.dataset.theme = theme.id;
    return () => {
      delete root.dataset.themeBase;
      delete root.dataset.theme;
    };
  }, [theme.base, theme.id]);

  return <ShellContext value={{ snapshot, live, state, dispatch, mode, setMode, open }}>
    <div className={`scene shell${showVideo ? "" : " image-mode"}${appearance.wallpaperMotion === false || svgActive || performanceMode ? " no-motion" : ""}`} data-theme={theme.id} data-theme-base={theme.base} data-material={material} data-app-inputs={appInputs} data-glass={glassEngine} data-scheme={scheme} data-perf={ultra ? "ultra" : undefined} data-maximized={windowMaximized ? "true" : undefined} data-launcher={launcherWindow ? "app" : undefined} data-inactive={inactive ? "true" : undefined} data-debug={debug ? "true" : undefined} data-animations={animations ? undefined : "off"} data-shell-build={snapshot.shellBuildId} data-shell-mode={mode} data-os-mode={osMode.mode} data-mobile={viewport.mobile ? "true" : undefined} data-orientation={viewport.mobile ? (viewport.landscape ? "landscape" : "portrait") : undefined} data-preferences-revision={preferences.preferences?.revision}>
      <div className="wallpaper media-wall" aria-hidden="true">
        {showVideo
          ? media.video ? <WallpaperVideo key={media.video} src={media.video} />
            : null
          : mediaSrc ? <img key={mediaSrc} className="wallpaper-image" src={mediaSrc} alt="" />
            : null}
      </div>
      {debug ? <div className="debug-badge" aria-hidden="true">debug · {glassBackend} · {glassQuality} · {glassOff ? "glass off" : "glass on"}</div> : null}
      <ToastStack />
      <MobileOrientationHint open={desktopHint} onDismiss={() => { setDesktopHint(false); setHintDismissed(true); }} />
      <FrontendUpdateNotice />
      <div className="shell__workspace">
        {theme.slots?.menubar ? <SlotRegion name="menubar" /> : <MenuBar />}
        <div className="workspace-surfaces">
          {mode === "desktop"
            ? theme.slots?.desktop
              ? <SlotRegion name="desktop" />
              : <DesktopLayout><RouteBoundary location={path}>{route.presentation === "window" ? <HomePage /> : <ShellRoutes />}</RouteBoundary></DesktopLayout>
            : theme.slots?.launcher
              ? <SlotRegion name="launcher" />
              : <LauncherLayout><RouteBoundary location={path}>{route.presentation === "window" ? null : <ShellRoutes />}</RouteBoundary></LauncherLayout>}
          <DesktopWindows animations={animations} />
        </div>
        <CommandPalette />
      </div>
      {theme.slots?.dock ? <SlotRegion name="dock" /> : variants.shellLayout === "taskbar" ? <Taskbar /> : <OsDock />}
      {emergencyOpen ? <EmergencyShell /> : null}
      {paletteOpen ? <KeyPalette /> : null}
    </div>
  </ShellContext>;
}
