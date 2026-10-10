import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "react-router";
import { useTheme } from "@rumahl/ui";
import type { BridgeTheme } from "@rumahl/contracts/bridge";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { useWindowLocation } from "../routing/routes";
import { useAppCatalog } from "./AppCatalog";
import { attachAppBridge, postAppBridgeEvent, type AppBridgeHandlers } from "./bridge";
import { attachProviderChannel } from "./provider-channel";
import { invokeProvider, registerProviderChannel } from "./providers";
import { readShellTheme } from "./shell-theme";
import { BRIDGE_CAPABILITY_LIST } from "@rumahl/contracts/bridge";
import { controlAppRuntime, invokeCapability as requestCapabilityInvocation, launchApp, type InstalledApp } from "./client";
import { setLaunching } from "./launching";
import { showToast } from "../shell/toasts";
import { Button } from "../components/Button";
import { UnavailablePage } from "../pages/UnavailablePage";
import "../routing/route-failure.css";

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
  // Reachability preflight: a network-level failure (the app's server is down)
  // rejects a no-cors fetch, so the shell shows its own unavailable state
  // instead of the browser's error page inside the iframe. HTTP error pages
  // still load as content; only connection/DNS failures are caught here.
  useEffect(() => {
    if (!frame) return;
    const controller = new AbortController();
    fetch(frame, { mode: "no-cors", cache: "no-store", signal: controller.signal }).catch(() => {
      if (!controller.signal.aborted) { cancel.current(); setFrame(null); setFailed(true); }
    });
    return () => controller.abort();
  }, [frame]);
  const readTheme = useCallback((): BridgeTheme => readShellTheme(tokens as Record<string, string>), [tokens]);
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
    },
    invokeCapability: async (capability, resource) => {
      if (!live) throw new Error("app unavailable");
      const result = await requestCapabilityInvocation(live.request, capability, resource, new AbortController().signal);
      // A web provider has no live channel: deliver the invocation to its iframe.
      if (result.browser) {
        const value = await invokeProvider(result.browser.appId, capability, resource);
        return { capability, outcome: "invoked", ...(value === undefined ? {} : { result: value }) };
      }
      return { capability: result.capability, outcome: result.outcome, ...(result.result === undefined ? {} : { result: result.result }) };
    }
  };
  useEffect(() => {
    const iframe = frameRef.current;
    const handlers = bridge.current;
    if (!iframe || !handlers) return;
    const appBridge = attachAppBridge(iframe, handlers);
    // Open app windows also act as capability providers for other apps. Delivery
    // waits for the app's handshake so it does not race provider registration.
    const provider = attachProviderChannel(iframe, { ready: appBridge.ready });
    const unregister = registerProviderChannel(app.id, provider);
    return () => { appBridge.dispose(); unregister(); provider.dispose(); };
  }, [frame, app.id]);
  // Push `os.theme.changed` whenever the shell switches appearance: once when
  // the theme is (re)bound and on any DOM scheme/theme attribute change.
  useEffect(() => {
    postAppBridgeEvent(frameRef.current, "os.theme.changed", readTheme());
    const scene = document.querySelector(".scene");
    if (!scene) return;
    const observer = new MutationObserver(() => postAppBridgeEvent(frameRef.current, "os.theme.changed", readTheme()));
    observer.observe(scene, { attributes: true, attributeFilter: ["data-scheme", "data-theme"] });
    return () => observer.disconnect();
  }, [readTheme, frame]);
  // On-demand apps start when their window opens and stop when it closes;
  // always-on services keep running in the background.
  useEffect(() => {
    if (!live || app.lifecycle === "always-on") return;
    const controller = new AbortController();
    void controlAppRuntime(live.request, app, "start", controller.signal).catch(() => undefined);
    return () => {
      controller.abort();
      void controlAppRuntime(live.request, app, "stop", new AbortController().signal).catch(() => undefined);
    };
  }, [live, app.id, app.installationId, app.lifecycle]);
  if (failed) return <section className="route-failure" aria-label={app.title}>
    <div className="route-failure__content">
      <div className="route-failure__icon" aria-hidden="true">
        <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="7" y="9" width="34" height="30" rx="7" /><path d="M7 18h34M13 13.5h.01M17 13.5h.01M24 24v6M24 34h.01" />
        </svg>
      </div>
      <div role="alert"><h2>{app.title}</h2><p>{t("apps.launchFailed")}</p></div>
      <div className="route-failure__actions">
        <Button onClick={() => setRetry((value) => value + 1)}>{t("apps.retry")}</Button>
      </div>
    </div>
  </section>;
  if (!frame) return <div className="installed-app-loading"><span className="visually-hidden">{t("apps.loading")}</span></div>;
  const url = new URL(frame);
  // Key the fragment off this window's location, not the shared browser URL, so
  // focusing or switching windows never changes the iframe source.
  const query = new URLSearchParams(windowLocation ? (windowLocation.split("?")[1]?.split("#")[0] ?? "") : "");
  query.delete("mode");
  // App navigation is a fragment on the authorized entrypoint, never an asset path.
  url.hash = `/${params["*"] ?? ""}${query.size ? `?${query}` : ""}`;
  return <iframe ref={frameRef} className="installed-app-frame" sandbox="allow-scripts allow-forms" referrerPolicy="no-referrer"
    title={app.title} src={url.href} onLoad={() => { loaded.current = true; }} onError={() => { cancel.current(); setFrame(null); setFailed(true); }} />;
}
