import { useEffect, useRef, useState } from "react";
import { useLocation, useParams } from "react-router";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { useAppCatalog } from "./AppCatalog";
import { launchApp, type InstalledApp } from "./client";
import { UnavailablePage } from "../pages/UnavailablePage";

export function InstalledAppHost() {
  const { appId = "" } = useParams();
  const { apps, status } = useAppCatalog();
  const { t } = useI18n();
  if (status === "loading") return <p role="status">{t("apps.loading")}</p>;
  const app = apps.find((item) => item.id === appId);
  if (!app?.launchable) return <UnavailablePage app />;
  return <AppFrame key={`${app.installationId}:${app.version}`} app={app} />;
}
function AppFrame({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t } = useI18n();
  const params = useParams();
  const location = useLocation();
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
  if (failed) return <div role="status"><p>{t("apps.launchFailed")}</p><button type="button" onClick={() => setRetry((value) => value + 1)}>{t("apps.retry")}</button></div>;
  if (!frame) return <p role="status">{t("apps.loading")}</p>;
  const url = new URL(frame);
  const query = new URLSearchParams(location.search);
  query.delete("mode");
  // App navigation is a fragment on the authorized entrypoint, never an asset path.
  url.hash = `/${params["*"] ?? ""}${query.size ? `?${query}` : ""}`;
  return <iframe className="installed-app-frame" sandbox="allow-scripts" referrerPolicy="no-referrer"
    title={app.title} src={url.href} onError={() => { cancel.current(); setFrame(null); setFailed(true); }} />;
}
