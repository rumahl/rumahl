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
import {
  BRIDGE_CAPABILITY_LIST,
  BRIDGE_NOTIFICATION_ACTION_TOPIC,
  BRIDGE_PET_ACTION_TOPIC,
  BRIDGE_PET_INTERACTION_TOPIC,
  type BridgePetActionRegistration,
  type BridgeWeatherWarning
} from "@rumahl/contracts/bridge";
import {
  fetchAppSettings, invokeCapability as requestCapabilityInvocation, launchApp,
  readAppFileContent, requestAppNetwork, updateAppSettings, writeAppFileContent,
  NetworkConsentRequiredError, SafeModeError, type InstalledApp
} from "./client";
import { AppFilePickerDialog, type FilePickerRequest } from "./FilePickerDialog";
import { NetworkConsentDialog, type NetworkConsentRequest } from "./NetworkConsentDialog";
import { decodeBridgeBase64, encodeBridgeBase64 } from "./bridge";
import { clearWeatherWarnings, publishWeatherWarnings, registerWarningSource } from "./warnings-client";
import { setLaunching } from "./launching";
import {
  dispatchPetCommand,
  getPetPublicState,
  registerPetAction,
  subscribePetInteractions
} from "./pets/pet-extensions";
import { showToast } from "../shell/toasts";
import { Button, ButtonLink } from "../components/Button";
import { UnavailablePage } from "../pages/UnavailablePage";
import type { ShellRequest } from "../snapshot-client";
import "../routing/route-failure.css";

/**
 * Reports an app lifecycle event (launch failure, crash, unresponsiveness) to
 * the OS so it lands in the central audit log. Best-effort: never blocks the UI.
 */
function postReport(request: ShellRequest, appId: string, event: "launched" | "crashed" | "unresponsive" | "launch-failed" | "stopped", installationId?: string): void {
  void request("/api/v1/shell/report", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ appId, event, ...(installationId ? { installationId } : {}) }),
  }).catch(() => undefined);
}

export function InstalledAppHost() {
  const { appId = "" } = useParams();
  const { apps, status } = useAppCatalog();
  const { t } = useI18n();
  // No visible "Loading apps…" text: the launching app bounces in the dock.
  if (status === "loading") return <div className="installed-app-loading" role="status">
    <span className="installed-app-loading__spinner" aria-hidden="true" />
    <span className="visually-hidden">{t("apps.loading")}</span>
  </div>;
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
  // One consent dialog at a time; the app's bridge request parks here until
  // the user picked files or declined.
  const [filePicker, setFilePicker] = useState<FilePickerRequest | null>(null);
  const [networkConsent, setNetworkConsent] = useState<NetworkConsentRequest | null>(null);
  const [frame, setFrame] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [safeMode, setSafeMode] = useState(false);
  const cancel = useRef<() => void>(() => {});
  const loaded = useRef(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!live) return;
    const request = live.request;
    const demo = live.demo;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    let lease: string | undefined;
    cancel.current = () => { controller.abort(); clearTimeout(timer); };
    setFrame(null); setFailed(false); setSafeMode(false);
    async function renew() {
      try {
        const launch = await launchApp(request, app, window.location.origin, controller.signal, lease, demo);
        if (controller.signal.aborted) return;
        lease = launch.lease;
        setSafeMode(false);
        setFrame(launch.frameUrl);
        timer = setTimeout(() => { void renew(); }, launch.renewAfterSeconds * 1000);
      } catch (failure) {
        if (controller.signal.aborted) return;
        setFrame(null);
        if (failure instanceof SafeModeError) {
          setSafeMode(true);
          setFailed(false);
        } else {
          setFailed(true);
          postReport(request, app.id, "launch-failed");
        }
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
    if (!frame || !live) return;
    const request = live.request;
    loaded.current = false;
    const timer = setTimeout(() => { if (!loaded.current) { setFrame(null); setFailed(true); postReport(request, app.id, "unresponsive", app.installationId); } }, 15_000);
    return () => clearTimeout(timer);
  }, [frame, live, app.id]);
  // Reachability preflight: a network-level failure (the app's server is down)
  // rejects a no-cors fetch, so the shell shows its own unavailable state
  // instead of the browser's error page inside the iframe. HTTP error pages
  // still load as content; only connection/DNS failures are caught here.
  useEffect(() => {
    if (!frame || !live) return;
    const request = live.request;
    const controller = new AbortController();
    fetch(frame, { mode: "no-cors", cache: "no-store", signal: controller.signal }).catch(() => {
      if (!controller.signal.aborted) { cancel.current(); setFrame(null); setFailed(true); postReport(request, app.id, "unresponsive", app.installationId); }
    });
    return () => controller.abort();
  }, [frame, live, app.id]);
  const readTheme = useCallback((): BridgeTheme => readShellTheme(tokens as Record<string, string>), [tokens]);
  // App <-> OS bridge: a sandboxed, opaque-origin app can only reach the shell
  // through postMessage. Rebound only when the frame changes; the handlers are
  // read through a ref so a shell re-render never detaches the listener.
  const bridge = useRef<AppBridgeHandlers | null>(null);
  const petRegistrations = useRef(new Map<string, () => void>());
  bridge.current = {
    appId: app.id,
    appTitle: app.title,
    version: app.version,
    accountName: snapshot.user.displayName,
    mode,
    capabilities: app.capabilities ?? BRIDGE_CAPABILITY_LIST,
    theme: readTheme,
    notify: (notification) => {
      let notificationId = "";
      const buttons = notification.buttons?.map((button, index) => ({
        label: button.label,
        variant: index === 0 ? "primary" as const : "default" as const,
        onClick: () => {
          postAppBridgeEvent(frameRef.current, BRIDGE_NOTIFICATION_ACTION_TOPIC, {
            notificationId,
            actionId: button.id
          });
        }
      }));
      notificationId = showToast({
        ...(notification.title ? { title: notification.title } : {}),
        message: notification.message,
        ...(notification.description ? { description: notification.description } : {}),
        variant: notification.variant ?? (notification.emergency ? "error" : "info"),
        ...(notification.duration === undefined ? {} : { duration: notification.duration }),
        ...(buttons ? { buttons } : {}),
        ...(notification.meta ?? app.title ? { meta: notification.meta ?? app.title } : {}),
        ...(notification.image ? { image: notification.image } : {}),
        ...(notification.progress === undefined ? {} : { progress: notification.progress }),
        source: { appId: app.id, appTitle: app.title },
        ...(notification.emergency ? { emergency: true } : {})
      });
    },
    windowControl: (action) => {
      const id = `app:${app.id}`;
      if (action === "focus") dispatch({ type: "focus-window", id });
      else dispatch({ type: action === "minimize" ? "toggle-minimize" : "close-window", id });
      if (action !== "focus" && (mode === "launcher" || mode === "mobile")) open("/");
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
    },
    petState: () => getPetPublicState(),
    petInteract: (action) => getPetPublicState() !== null && dispatchPetCommand(action, `app:${app.id}`),
    petRegisterAction: (action: BridgePetActionRegistration) => {
      petRegistrations.current.get(action.id)?.();
      const unregister = registerPetAction({
        id: action.id,
        label: action.label,
        ...(action.icon ? { icon: action.icon } : {}),
        appId: app.id,
        invoke: () => postAppBridgeEvent(frameRef.current, BRIDGE_PET_ACTION_TOPIC, { id: action.id })
      });
      if (!unregister) return null;
      petRegistrations.current.set(action.id, unregister);
      return unregister;
    },
    petUnregisterAction: (id) => {
      const unregister = petRegistrations.current.get(id);
      if (!unregister) return false;
      unregister();
      petRegistrations.current.delete(id);
      return true;
    },
    ...(live ? {
      filesRequest: (access) => new Promise((resolve) => {
        setFilePicker({ access, resolve });
      }),
      filesRead: async (key) => {
        const { name, bytes } = await readAppFileContent(live.request, app, key, new AbortController().signal);
        const contentBase64 = encodeBridgeBase64(bytes);
        if (!contentBase64) throw new Error("file too large for the bridge");
        return { name, size: bytes.length, contentBase64 };
      },
      filesWrite: async (key, contentBase64) => {
        const bytes = decodeBridgeBase64(contentBase64);
        if (!bytes) throw new Error("invalid file content");
        return await writeAppFileContent(live.request, app, key, bytes, new AbortController().signal);
      },
      warningRegister: async () => {
        const source = await registerWarningSource(live.request, app.id, new AbortController().signal);
        return { source };
      },
      warningPublish: async (warnings: readonly BridgeWeatherWarning[]) => {
        const accepted = await publishWeatherWarnings(live.request, app.id, warnings, new AbortController().signal);
        return { accepted };
      },
      warningClear: async (ids?: string[]) => {
        const cleared = await clearWeatherWarnings(live.request, app.id, ids, new AbortController().signal);
        return { cleared };
      },
      settings: async () => {
        const settings = await fetchAppSettings(live.request, app, new AbortController().signal);
        return { values: settings.values, secrets: [...settings.secrets] };
      },
      setSettings: async (input) => {
        const settings = await updateAppSettings(live.request, app, input.values, new AbortController().signal);
        return { values: settings.values, secrets: [...settings.secrets] };
      },
      netRequest: async (input) => {
        const askConsent = (origin: string) => new Promise<boolean>((resolve) => {
          setNetworkConsent({ origin, resolve });
        });
        try {
          return await requestAppNetwork(live.request, app, input, new AbortController().signal);
        } catch (error) {
          if (!(error instanceof NetworkConsentRequiredError)) throw error;
          const granted = await askConsent(error.origin);
          if (!granted) throw new Error("the user declined network access", { cause: error });
          return await requestAppNetwork(live.request, app, input, new AbortController().signal);
        }
      }
    } : {})
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
  // Pet menu actions belong to the app window that registered them. Closing
  // or reloading the window drops them; the shell never keeps app code alive.
  useEffect(() => () => {
    for (const unregister of petRegistrations.current.values()) unregister();
    petRegistrations.current.clear();
  }, [app.id, frame]);
  // Apps with the pet capability may react to every companion interaction.
  useEffect(() => {
    if (!(app.capabilities ?? BRIDGE_CAPABILITY_LIST).includes("com.rumahl.pets.interact")) return;
    return subscribePetInteractions((interaction) => {
      postAppBridgeEvent(frameRef.current, BRIDGE_PET_INTERACTION_TOPIC, interaction);
    });
  }, [app.id, frame, app.capabilities]);
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
  if (safeMode) return <section className="route-failure" aria-label={app.title}>
    <div className="route-failure__content">
      <div className="route-failure__icon" aria-hidden="true">
        <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M24 6 8 13v10c0 9 6.8 15.4 16 19 9.2-3.6 16-10 16-19V13Z" /><path d="M17 24h14M24 17v14" />
        </svg>
      </div>
      <div role="alert"><h2>{t("apps.safeModeTitle", { title: app.title })}</h2><p>{t("apps.safeModeHelp")}</p></div>
      <div className="route-failure__actions">
        <Button onClick={() => setRetry((value) => value + 1)}>{t("apps.retry")}</Button>
        <ButtonLink variant="secondary" href="/recovery">{t("apps.openRecovery")}</ButtonLink>
      </div>
    </div>
  </section>;
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
  if (!frame) return <div className="installed-app-loading" role="status">
    <span className="installed-app-loading__spinner" aria-hidden="true" />
    <span className="visually-hidden">{t("apps.loading")}</span>
  </div>;
  const url = new URL(frame);
  // Key the fragment off this window's location, not the shared browser URL, so
  // focusing or switching windows never changes the iframe source.
  const query = new URLSearchParams(windowLocation ? (windowLocation.split("?")[1]?.split("#")[0] ?? "") : "");
  query.delete("mode");
  // App navigation is a fragment on the authorized entrypoint, never an asset path.
  url.hash = `/${params["*"] ?? ""}${query.size ? `?${query}` : ""}`;
  return <>
    <iframe ref={frameRef} className="installed-app-frame" sandbox="allow-scripts" referrerPolicy="no-referrer"
      title={app.title} src={url.href} onLoad={() => { if (!loaded.current && live) postReport(live.request, app.id, "launched"); loaded.current = true; }} onError={() => { cancel.current(); setFrame(null); setFailed(true); if (live) postReport(live.request, app.id, "crashed", app.installationId); }} />
    {filePicker
      ? <AppFilePickerDialog
          app={app}
          request={filePicker}
          onDone={(result) => {
            filePicker.resolve(result);
            setFilePicker(null);
          }}
        />
      : null}
    {networkConsent
      ? <NetworkConsentDialog
          app={app}
          request={networkConsent}
          onDone={(granted) => {
            networkConsent.resolve(granted);
            setNetworkConsent(null);
          }}
        />
      : null}
  </>;
}
