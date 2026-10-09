import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "react-router";
import { useTheme } from "@rumahl/ui";
import type { BridgeTheme } from "@rumahl/contracts/bridge";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { useWindowLocation } from "../routing/routes";
import { useAppCatalog } from "./AppCatalog";
import { attachAppBridge, postAppBridgeEvent, type AppBridgeHandlers } from "./bridge";
import { BRIDGE_CAPABILITY_LIST } from "@rumahl/contracts/bridge";
import { launchApp, type InstalledApp } from "./client";
import { setLaunching } from "./launching";
import { showToast } from "../shell/toasts";
import { UnavailablePage } from "../pages/UnavailablePage";

export function InstalledAppHost() {
  const { appId = "" } = useParams();
  const { apps, status } = useAppCatalog();
  const { t } = useI18n();
  // No visible "Loading apps…" text: the launching app bounces in the dock.
  if (status === "loading") return <div className="installed-app-loading"><span className="visually-hidden">{t("apps.loading")}</span></div>;
  const app = apps.find((item) => item.id === appId);
  if (!app?.launchable) return <UnavailablePage app />;
  return <AppFrame key={`${app.installationId}:${app.version}`} app={app} />;
}
function AppFrame({ app }: { app: InstalledApp }) {
  const { live, snapshot, mode, dispatch, open } = useShell();
  const { tokens } = useTheme();
  const { t } = useI18n();
  const params = useParams();
  const windowLocation = useWindowLocation();
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [frame, setFrame] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const cancel = useRef<() => void>(() => {});
  const loaded = useRef(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    let lease: string | undefined;
    cancel.current = () => { controller.abort(); clearTimeout(timer); };
    setFrame(null); setFailed(false);
    async function renew() {
      try {
        const launch = await launchApp(live!.request, app, window.location.origin, controller.signal, lease);
        if (controller.signal.aborted) return;
        lease = launch.lease;
        setFrame(launch.frameUrl);
        timer = setTimeout(() => { void renew(); }, launch.renewAfterSeconds * 1000);
      } catch {
        if (!controller.signal.aborted) { setFrame(null); setFailed(true); }
      }
    }
    void renew();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [live, app.id, app.installationId, app.version, retry]);
  // Bounce the app's dock icon while the iframe is not ready yet.
  useEffect(() => () => setLaunching(app.id, false), [app.id]);
  useEffect(() => { setLaunching(app.id, !frame && !failed); }, [app.id, frame, failed]);
  // Watchdog: a cross-origin iframe that never loads (for example because the
  // app's server is down) would otherwise leave the browser's own error page on
  // screen. Fall back to the shell's unavailable state instead.
  useEffect(() => {
    if (!frame) return;
    loaded.current = false;
    const timer = setTimeout(() => { if (!loaded.current) { setFrame(null); setFailed(true); } }, 15_000);
    return () => clearTimeout(timer);
  }, [frame]);
  const readTheme = useCallback((): BridgeTheme => {
    const scene = document.querySelector<HTMLElement>(".scene");
    const scheme = scene?.dataset.scheme ?? (document.body.classList.contains("light") ? "light" : "dark");
    return {
      scheme: scheme === "light" ? "light" : "dark",
      accent: String(tokens["color.accent"] ?? ""),
      reducedMotion: typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
    };
  }, [tokens]);
  // App <-> OS bridge: a sandboxed, opaque-origin app can only reach the shell
  // through postMessage. Rebound only when the frame changes; the handlers are
  // read through a ref so a shell re-render never detaches the listener.
  const bridge = useRef<AppBridgeHandlers | null>(null);
  bridge.current = {
    appId: app.id,
    appTitle: app.title,
    version: app.version,
    accountName: snapshot.user.displayName,
    mode,
    capabilities: app.capabilities ?? BRIDGE_CAPABILITY_LIST,
    theme: readTheme,
    notify: (notification) => showToast({ ...(notification.title ? { title: notification.title } : {}), message: notification.message, variant: notification.variant ?? "info", duration: 5000 }),
    windowControl: (action) => {
      const id = `app:${app.id}`;
      if (action === "focus") dispatch({ type: "focus-window", id });
      else dispatch({ type: action === "minimize" ? "toggle-minimize" : "close-window", id });
      if (action !== "focus" && mode === "launcher") open("/");
    }
  };
  useEffect(() => {
    const iframe = frameRef.current;
    const handlers = bridge.current;
    if (!iframe || !handlers) return;
    return attachAppBridge(iframe, handlers);
  }, [frame]);
  // Push `os.theme.changed` whenever the shell switches appearance.
  useEffect(() => {
    const scene = document.querySelector(".scene");
    if (!scene) return;
    const observer = new MutationObserver(() => postAppBridgeEvent(frameRef.current, "os.theme.changed", readTheme()));
    observer.observe(scene, { attributes: true, attributeFilter: ["data-scheme", "data-theme"] });
    return () => observer.disconnect();
  }, [readTheme]);
  if (failed) return <div role="status"><p>{t("apps.launchFailed")}</p><button type="button" onClick={() => setRetry((value) => value + 1)}>{t("apps.retry")}</button></div>;
  if (!frame) return <div className="installed-app-loading"><span className="visually-hidden">{t("apps.loading")}</span></div>;
  const url = new URL(frame);
  // Key the fragment off this window's location, not the shared browser URL, so
  // focusing or switching windows never changes the iframe source.
  const query = new URLSearchParams(windowLocation ? (windowLocation.split("?")[1]?.split("#")[0] ?? "") : "");
  query.delete("mode");
  // App navigation is a fragment on the authorized entrypoint, never an asset path.
  url.hash = `/${params["*"] ?? ""}${query.size ? `?${query}` : ""}`;
  return <iframe ref={frameRef} className="installed-app-frame" sandbox="allow-scripts" referrerPolicy="no-referrer"
    title={app.title} src={url.href} onLoad={() => { loaded.current = true; }} onError={() => { cancel.current(); setFrame(null); setFailed(true); }} />;
}
