import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { useI18n } from "../i18n";
import { useOsModePolicy } from "../preferences/OsMode";
import { browserProfile } from "../preferences/storage";
import { useShell } from "../shell/ShellContext";
import { UnavailablePage } from "../pages/UnavailablePage";
import { useAppCatalog } from "./AppCatalog";
import {
  fetchAppData,
  fetchAppSettings,
  type AppData,
  type AppSetting,
  type AppSettings,
  type InstalledApp,
} from "./client";

/**
 * Per-app settings. The manifest is the trusted, signed source of the first
 * section: every setting is rendered with its title, description and typed
 * control. Extra configuration and the app's private data directory are a
 * second section that only appears from advanced mode upwards.
 */
export function InstalledAppSettings() {
  const { appId = "" } = useParams();
  const { apps, status } = useAppCatalog();
  const { t } = useI18n();
  if (status === "loading") return <p role="status">{t("apps.loading")}</p>;
  const app = apps.find((item) => item.id === appId);
  if (!app) return <UnavailablePage app />;
  return <AppSettingsBody key={`${app.installationId}:${app.version}`} app={app} />;
}

function AppSettingsBody({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t } = useI18n();
  const policy = useOsModePolicy();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setSettings(null);
    setFailed(false);
    fetchAppSettings(live.request, app, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setSettings(value); })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);

  if (failed) return <main className="route-message" role="alert"><h1>{app.title}</h1><p>{t("appSettings.unavailable")}</p></main>;
  if (!settings) return <p role="status">{t("appSettings.loading")}</p>;
  return <main className="app-settings" data-advanced={policy.advancedSettings ? "true" : "false"}>
    <h1>{app.title}</h1>
    <section aria-labelledby="app-settings-manifest">
      <h2 id="app-settings-manifest">{t("appSettings.manifestTitle")}</h2>
      {settings.manifest.length === 0
        ? <p>{t("appSettings.manifestEmpty")}</p>
        : <dl>{settings.manifest.map((setting) => <SettingRow key={setting.key} setting={setting} />)}</dl>}
    </section>
    {policy.advancedSettings
      ? <>
          <section aria-labelledby="app-settings-extended">
            <h2 id="app-settings-extended">{t("appSettings.extendedTitle")}</h2>
            <p>{t("appSettings.extendedEmpty")}</p>
          </section>
          <AppDataBrowser app={app} />
        </>
      : null}
  </main>;
}

function AppDataBrowser({ app }: { app: InstalledApp }) {
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
  return <section aria-labelledby="app-settings-data">
    <h2 id="app-settings-data">{t("appSettings.dataTitle")}</h2>
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
  </section>;
}

function SettingRow({ setting }: { setting: AppSetting }) {
  const { t } = useI18n();
  return <div className="app-setting" data-setting-key={setting.key}>
    <dt>
      {setting.title}
      {setting.required ? <span className="app-setting__required">{t("appSettings.required")}</span> : null}
    </dt>
    <dd className="app-setting__type">{t(`appSettings.type.${setting.type}`)}</dd>
    {setting.description ? <dd className="app-setting__description">{setting.description}</dd> : null}
    {setting.type === "select"
      ? <dd className="app-setting__options"><ul>{setting.options.map((option) => <li key={option.value}>{option.label}</li>)}</ul></dd>
      : null}
  </div>;
}
