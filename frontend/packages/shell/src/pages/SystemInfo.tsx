import { useTheme } from "@rumahl/ui";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";

export function SystemInfo() {
  const { t, locale } = useI18n();
  const { snapshot, mode } = useShell();
  const { theme } = useTheme();
  const unavailable = t("settings.system.unavailable");
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
      <InfoRow label={t("settings.system.os")} value="rumahl OS" />
      <InfoRow label={t("settings.system.build")} value={snapshot.shellBuildId} mono />
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
  </>;
}

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return <div className="setting setting--value">
    <div className="copy"><strong>{label}</strong></div>
    <div className="visual">{mono ? <code>{value}</code> : <span>{value}</span>}</div>
  </div>;
}
