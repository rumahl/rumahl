import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { browserProfile } from "../preferences/storage";
import { useShell } from "../shell/ShellContext";
import { fetchAppStorage, type AppStorage as Storage, type InstalledApp } from "./client";

/**
 * Measured storage of an app's private data directory against its configured
 * quota. Advanced-mode diagnostics, like the data browser it belongs to; the
 * server enforces the same gate.
 */
export function AppStorage({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t, locale } = useI18n();
  const [storage, setStorage] = useState<Storage | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setStorage(null);
    setFailed(false);
    fetchAppStorage(live.request, app, browserProfile().id, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setStorage(value); })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);
  const format = (bytes: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / (1024 * 1024));
  return <div className="app-storage" data-app-id={app.id}>
    <h2>{t("appSettings.storageTitle")}</h2>
    {failed
      ? <p role="alert">{t("appSettings.storageUnavailable")}</p>
      : !storage
        ? <p role="status">{t("appSettings.storageLoading")}</p>
        : <>
            <p>{t("appSettings.storageUsage", { used: format(storage.usedBytes), quota: format(storage.quotaBytes) })}</p>
            <progress
              max={storage.quotaBytes}
              value={Math.min(storage.usedBytes, storage.quotaBytes)}
              aria-label={t("appSettings.storageTitle")}
            />
          </>}
  </div>;
}
