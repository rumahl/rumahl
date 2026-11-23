import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { useI18n } from "../i18n";
import { useOsModePolicy } from "../preferences/OsMode";
import { useShell } from "../shell/ShellContext";
import { UnavailablePage } from "../pages/UnavailablePage";
import { AppErrorCard } from "../components/AppErrorCard";
import { LoadingStatus } from "../components/LoadingStatus";
import { useAppCatalog } from "./AppCatalog";
import { AppDataBrowser } from "./AppDataBrowser";
import { AppImageEnvironmentView } from "./AppImageEnvironmentView";
import { AppPermissions } from "./AppPermissions";
import { AppStorage } from "./AppStorage";
import { AppNetworkAccess } from "./AppNetworkAccess";
import { fetchAppSettings, updateAppSettings, type AppSetting, type AppSettings, type InstalledApp } from "./client";

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
  if (status === "loading") return <LoadingStatus label={t("apps.loading")} />;
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
  const [draft, setDraft] = useState<Record<string, string | number | boolean>>({});
  const [secretDraft, setSecretDraft] = useState<Record<string, string>>({});
  const [clearedSecrets, setClearedSecrets] = useState<ReadonlySet<string>>(new Set());
  const [saving, setSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setSettings(null);
    setFailed(false);
    fetchAppSettings(live.request, app, controller.signal)
      .then((value) => {
        if (controller.signal.aborted) return;
        setSettings(value);
        setDraft({ ...(value.values ?? {}) });
        setSecretDraft({});
        setClearedSecrets(new Set());
      })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);

  const save = async () => {
    if (!live || !settings || saving) return;
    const values: Record<string, string | number | boolean | null> = {};
    for (const setting of settings.manifest) {
      if (setting.type === "secret") {
        const secret = secretDraft[setting.key] ?? "";
        if (clearedSecrets.has(setting.key)) values[setting.key] = null;
        else if (secret.length > 0) values[setting.key] = secret;
      } else {
        const value = draft[setting.key];
        if (value !== undefined) values[setting.key] = value;
      }
    }
    setSaving(true);
    setSaveFailed(false);
    setSaved(false);
    try {
      const next = await updateAppSettings(live.request, app, values, new AbortController().signal);
      setSettings(next);
      setDraft({ ...next.values });
      setSecretDraft({});
      setClearedSecrets(new Set());
      setSaved(true);
    } catch {
      setSaveFailed(true);
    } finally {
      setSaving(false);
    }
  };

  if (failed) return <AppErrorCard title={app.title} message={t("appSettings.unavailable")} />;
  if (!settings) return <LoadingStatus label={t("appSettings.loading")} />;
  return <main className="app-settings" data-advanced={policy.advancedSettings ? "true" : "false"}>
    <h1>{app.title}</h1>
    <AppPermissions app={app} />
    <AppNetworkAccess app={app} />
    <section aria-labelledby="app-settings-manifest">
      <h2 id="app-settings-manifest">{t("appSettings.manifestTitle")}</h2>
      {settings.manifest.length === 0
        ? <p>{t("appSettings.manifestEmpty")}</p>
        : <>
            <dl>{settings.manifest.map((setting) => <SettingRow
              key={setting.key}
              setting={setting}
              value={draft[setting.key]}
              secret={secretDraft[setting.key] ?? ""}
              configured={(settings.secrets ?? []).includes(setting.key)}
              cleared={clearedSecrets.has(setting.key)}
              onValue={(value) => {
                setSaved(false);
                setDraft((current) => ({ ...current, [setting.key]: value }));
              }}
              onSecret={(value) => {
                setSaved(false);
                setClearedSecrets((current) => {
                  if (!current.has(setting.key)) return current;
                  const next = new Set(current);
                  next.delete(setting.key);
                  return next;
                });
                setSecretDraft((current) => ({ ...current, [setting.key]: value }));
              }}
              onClearSecret={() => {
                setSaved(false);
                setSecretDraft((current) => ({ ...current, [setting.key]: "" }));
                setClearedSecrets((current) => new Set(current).add(setting.key));
              }}
            />)}</dl>
            <div className="app-settings__actions">
              <button type="button" className="app-settings__save" disabled={saving} onClick={() => void save()}>
                {saving ? t("appSettings.saving") : t("appSettings.save")}
              </button>
              {saved ? <p role="status">{t("appSettings.saved")}</p> : null}
              {saveFailed ? <p role="alert">{t("appSettings.saveFailed")}</p> : null}
            </div>
          </>}
    </section>
    {policy.advancedSettings
      ? <>
          <section aria-labelledby="app-settings-extended">
            <h2 id="app-settings-extended">{t("appSettings.extendedTitle")}</h2>
            <p>{t("appSettings.extendedEmpty")}</p>
          </section>
          {app.runtime === "container" ? <AppImageEnvironmentView app={app} /> : null}
          <AppStorage app={app} />
          <AppDataBrowser app={app} />
        </>
      : null}
  </main>;
}

function SettingRow({ setting, value, secret, configured, cleared, onValue, onSecret, onClearSecret }: {
  setting: AppSetting;
  value: string | number | boolean | undefined;
  secret: string;
  configured: boolean;
  cleared: boolean;
  onValue: (value: string | number | boolean) => void;
  onSecret: (value: string) => void;
  onClearSecret: () => void;
}) {
  const { t } = useI18n();
  const inputId = `app-setting-${setting.key}`;
  let control;
  if (setting.type === "boolean") {
    control = <input
      id={inputId}
      type="checkbox"
      checked={value === true}
      onChange={(event) => onValue(event.target.checked)}
    />;
  } else if (setting.type === "number") {
    control = <input
      id={inputId}
      type="number"
      value={typeof value === "number" ? value : ""}
      onChange={(event) => onValue(Number(event.target.value))}
    />;
  } else if (setting.type === "select") {
    control = <select id={inputId} value={typeof value === "string" ? value : ""} onChange={(event) => onValue(event.target.value)}>
      <option value="">{t("appSettings.choose")}</option>
      {setting.options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
    </select>;
  } else if (setting.type === "secret") {
    control = <>
      <input
        id={inputId}
        type="password"
        autoComplete="new-password"
        placeholder={configured && !cleared ? t("appSettings.secretConfigured") : t("appSettings.secretEmpty")}
        value={secret}
        onChange={(event) => onSecret(event.target.value)}
      />
      {configured && !cleared
        ? <button type="button" className="app-setting__clear" onClick={onClearSecret}>
            {t("appSettings.clearSecret")}
          </button>
        : null}
    </>;
  } else {
    control = <input
      id={inputId}
      type="text"
      value={typeof value === "string" ? value : ""}
      onChange={(event) => onValue(event.target.value)}
    />;
  }
  return <div className="app-setting" data-setting-key={setting.key}>
    <dt>
      <label htmlFor={inputId}>{setting.title}</label>
      {setting.required ? <span className="app-setting__required">{t("appSettings.required")}</span> : null}
    </dt>
    <dd className="app-setting__type">{t(`appSettings.type.${setting.type}`)}</dd>
    {setting.description ? <dd className="app-setting__description">{setting.description}</dd> : null}
    <dd className="app-setting__control">{control}</dd>
  </div>;
}
