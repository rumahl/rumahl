import { useEffect, useState } from "react";
import { useTheme } from "@rumahl/ui";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { fetchSensors, type Sensors } from "../apps/recovery-client";
import { requestPlatformConfig, type PlatformConfig } from "../preferences/client";
import { browserProfile } from "../preferences/storage";
import { useOsModePolicy } from "../preferences/OsMode";
import { buildLabel, osName } from "../system-labels";

export function SystemInfo() {
  const { t, locale } = useI18n();
  const { snapshot, mode, live } = useShell();
  const demo = Boolean(live?.demo);
  const { theme } = useTheme();
  const [sensors, setSensors] = useState<Sensors | null>(null);
  const [sensorsError, setSensorsError] = useState(false);
  // The extended configuration view is the advanced sibling of the host file
  // browser; the server enforces the same gate.
  const extended = useOsModePolicy().viewPlatformConfig;
  const [config, setConfig] = useState<PlatformConfig | null>(null);
  const unavailable = t("settings.system.unavailable");
  useEffect(() => {
    if (!live?.request) return;
    let canceled = false;
    const load = () => {
      fetchSensors(live.request).then((value) => {
        if (!canceled) { setSensors(value); setSensorsError(false); }
      }).catch(() => {
        if (!canceled) { setSensors(null); setSensorsError(true); }
      });
    };
    load();
    const timer = window.setInterval(load, 15_000);
    return () => { canceled = true; window.clearInterval(timer); };
  }, [live?.request]);
  useEffect(() => {
    if (!live?.request || !extended) return;
    const controller = new AbortController();
    requestPlatformConfig(live.request, browserProfile().id, controller.signal).then((value) => {
      setConfig(value);
    }).catch(() => {
      setConfig(null);
    });
    return () => controller.abort();
  }, [live?.request, extended]);
  const format = (value: number | null | undefined) => {
    if (!value) return unavailable;
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? unavailable : new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
  };
  const protection = snapshot.systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  const lastActivity = snapshot.systemStatus.lastActivityAtUnixMs ?? null;

  return <>
    <h3 className="section-title">{t("settings.system.info")}</h3>
    <div className="settings-group">
      <InfoRow label={t("settings.system.os")} value={osName(demo)} />
      <InfoRow label={t("settings.system.build")} value={buildLabel(snapshot.shellBuildId, demo)} mono />
      <InfoRow label={t("settings.system.revision")} value={snapshot.revision || unavailable} mono />
      <InfoRow label={t("settings.system.mode")} value={t(`mode.${mode}`)} />
      <InfoRow label={t("settings.system.theme")} value={theme.name} />
      <InfoRow label={t("settings.system.language")} value={snapshot.user.locale || unavailable} />
    </div>

    <h3 className="section-title">{t("settings.system.status")}</h3>
    <div className="settings-group">
      <InfoRow label={t("settings.system.protection")} value={protection} />
      <InfoRow label={t("settings.system.installedApps")} value={String(snapshot.systemStatus.installedAppCount)} />
      <InfoRow label={t("settings.system.observed")} value={format(snapshot.systemStatus.observedAtUnixMs)} />
      <InfoRow label={t("settings.system.lastActivity")} value={format(lastActivity)} />
    </div>

    <h3 className="section-title">{t("settings.system.hardware")}</h3>
    <div className="settings-group">
      <InfoRow label={t("settings.system.hostname")} value={sensors?.host ?? unavailable} mono />
      <InfoRow label={t("settings.system.cpuCores")} value={sensors ? String(sensors.cores) : unavailable} />
      <InfoRow label={t("settings.system.uptime")} value={sensors?.uptime !== null && sensors?.uptime !== undefined ? formatUptime(Math.floor(sensors.uptime)) : unavailable} />
      {sensors?.metrics.map((metric) => <InfoRow key={metric.id} label={metric.label} value={`${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(metric.value)} ${metric.unit}`.trim()} />)}
      {sensorsError ? <InfoRow label={t("settings.system.sensors")} value={unavailable} /> : null}
    </div>

    {extended ? <>
      <h3 className="section-title">{t("settings.system.config")}</h3>
      <div className="settings-group">
        {config && config.entries.length > 0
          ? config.entries.map((entry) => <InfoRow key={entry.name} label={entry.name} value={entry.value} mono />)
          : <InfoRow label={t("settings.system.configSource")} value={unavailable} />}
      </div>
    </> : null}
  </>;
}

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  return days > 0 ? `${days}d ${hours}h ${minutes}m` : hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return <div className="setting setting--value">
    <div className="copy"><strong>{label}</strong></div>
    <div className="visual">{mono ? <code>{value}</code> : <span>{value}</span>}</div>
  </div>;
}
