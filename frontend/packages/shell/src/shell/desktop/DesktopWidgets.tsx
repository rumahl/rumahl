import type { ShellSystemStatus } from "@rumahl/contracts";
import { useI18n } from "../../i18n";
import { useClock } from "../clock";

export function DesktopWidgets({ systemStatus }: { systemStatus: ShellSystemStatus }) {
  const { t, locale } = useI18n();
  const now = useClock();
  const protection = systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  return <section className="desktop-widgets" aria-label={t("desktop.widgets.label")}>
    <article className="desktop-widget desktop-widget--clock">
      <p className="desktop-widget__eyebrow">{t("desktop.widgets.clock")}</p>
      <strong className="desktop-widget__time">{now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "--:--"}</strong>
      <span>{now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : ""}</span>
    </article>
    <article className="desktop-widget desktop-widget--status">
      <p className="desktop-widget__eyebrow">{t("status.title")}</p>
      <strong>{protection}</strong>
      <span>{t("status.apps.label")}: {systemStatus.installedAppCount}</span>
    </article>
  </section>;
}
