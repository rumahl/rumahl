import { useRef } from "react";
import type { ShellSystemStatus } from "@rumahl/contracts";
import { useI18n } from "../../i18n";
import { useClock } from "../clock";
import { useGlassHost } from "../../glass-engine/useGlassEngine";
import { GLASS_MATERIALS } from "@rumahl/ui/glass";


export function DesktopWidgets({ systemStatus }: { systemStatus: ShellSystemStatus }) {
  const { t, locale } = useI18n();
  const now = useClock();
  const clockRef = useRef<HTMLElement>(null);
  const statusRef = useRef<HTMLElement>(null);
  useGlassHost(clockRef, GLASS_MATERIALS.widget);
  useGlassHost(statusRef, GLASS_MATERIALS.widget);
  const protection = systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  return <section className="desktop-widgets widgets" aria-label={t("desktop.widgets.label")}>
    <article ref={clockRef} className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--clock">
      <p className="desktop-widget__eyebrow">{t("desktop.widgets.clock")}</p>
      <strong className="desktop-widget__time">{now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "--:--"}</strong>
      <span>{now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : ""}</span>
    </article>
    <article ref={statusRef} className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--status">
      <p className="desktop-widget__eyebrow">{t("status.title")}</p>
      <strong>{protection}</strong>
      <span>{t("status.apps.label")}: {systemStatus.installedAppCount}</span>
    </article>
  </section>;
}
