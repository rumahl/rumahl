import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { useWindowLocation } from "../routing/routes";
import { useAppCatalog } from "./AppCatalog";
import { launchApp, type InstalledApp } from "./client";
import { setLaunching } from "./launching";
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
  const { live } = useShell();
  const { t } = useI18n();
  const params = useParams();
  const windowLocation = useWindowLocation();
  const [frame, setFrame] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const cancel = useRef<() => void>(() => {});
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
  if (failed) return <div role="status"><p>{t("apps.launchFailed")}</p><button type="button" onClick={() => setRetry((value) => value + 1)}>{t("apps.retry")}</button></div>;
  if (!frame) return <div className="installed-app-loading"><span className="visually-hidden">{t("apps.loading")}</span></div>;
  const url = new URL(frame);
  // Key the fragment off this window's location, not the shared browser URL, so
  // focusing or switching windows never changes the iframe source.
  const query = new URLSearchParams(windowLocation ? (windowLocation.split("?")[1]?.split("#")[0] ?? "") : "");
  query.delete("mode");
  // App navigation is a fragment on the authorized entrypoint, never an asset path.
  url.hash = `/${params["*"] ?? ""}${query.size ? `?${query}` : ""}`;
  return <iframe className="installed-app-frame" sandbox="allow-scripts" referrerPolicy="no-referrer"
    title={app.title} src={url.href} onError={() => { cancel.current(); setFrame(null); setFailed(true); }} />;
}
