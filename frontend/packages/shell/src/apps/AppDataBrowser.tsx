import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { browserProfile } from "../preferences/storage";
import { useShell } from "../shell/ShellContext";
import { fetchAppData, type AppData, type InstalledApp } from "./client";

/**
 * Read-only browser for an app's private data directory. Only rendered in
 * advanced mode or above; the server independently enforces that gate.
 */
export function AppDataBrowser({ app, heading = true }: { app: InstalledApp; heading?: boolean }) {
  const { live } = useShell();
  const { t } = useI18n();
  const device = browserProfile().id;
  const [path, setPath] = useState("");
  const [data, setData] = useState<AppData | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setData(null);
    setFailed(false);
    fetchAppData(live.request, app, path, device, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setData(value); })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version, path, device]);
  const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
  return <div className="app-data" data-app-id={app.id}>
    {heading ? <h2>{t("appSettings.dataTitle")}</h2> : null}
    <p className="app-settings__path">{path || "/"}</p>
    {path ? <button type="button" onClick={() => setPath(parent)}>{t("appSettings.dataUp")}</button> : null}
    {failed
      ? <p role="alert">{t("appSettings.dataUnavailable")}</p>
      : !data
        ? <p role="status">{t("apps.loading")}</p>
        : data.kind === "directory"
          ? (data.entries.length === 0
              ? <p>{t("appSettings.dataEmpty")}</p>
              : <ul className="app-data-list">{data.entries.map((entry) =>
                  <li key={entry.name}>
                    <button type="button" onClick={() => setPath(path ? `${path}/${entry.name}` : entry.name)}>
                      {entry.name}{entry.directory ? "/" : ` · ${entry.size}`}
                    </button>
                  </li>)}</ul>)
          : <pre className="app-data-preview">{data.text ?? t("appSettings.dataBinary", { size: data.size })}</pre>}
  </div>;
}
