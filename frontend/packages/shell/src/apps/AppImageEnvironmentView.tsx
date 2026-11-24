import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { browserProfile } from "../preferences/storage";
import { useShell } from "../shell/ShellContext";
import { LoadingStatus } from "../components/LoadingStatus";
import { fetchAppImageEnvironment, type AppImageEnvironment, type InstalledApp } from "./client";

/**
 * The recorded environment of a container app's image, captured at import
 * time. Only rendered in advanced mode or above; the server independently
 * enforces that gate, and names that could carry a credential never reach
 * this component at all.
 */
export function AppImageEnvironmentView({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t } = useI18n();
  const [environment, setEnvironment] = useState<AppImageEnvironment | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setEnvironment(null);
    setFailed(false);
    fetchAppImageEnvironment(live.request, app, browserProfile().id, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setEnvironment(value); })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);
  return <div className="app-image" data-app-id={app.id}>
    <h2>{t("appSettings.imageTitle")}</h2>
    {failed
      ? <p role="alert">{t("appSettings.imageUnavailable")}</p>
      : !environment
        ? <LoadingStatus label={t("apps.loading")} />
        : <>
            <p className="app-settings__path">{environment.image ?? t("appSettings.imageNone")}</p>
            {environment.entries.length === 0
              ? <p>{t("appSettings.imageEmpty")}</p>
              : <dl className="app-image-list">{environment.entries.map((entry) =>
                  <div className="app-setting" data-image-variable={entry.name} key={entry.name}>
                    <dt>{entry.name}</dt>
                    <dd className="app-setting__type"><code>{entry.value}</code></dd>
                  </div>)}</dl>}
          </>}
  </div>;
}
