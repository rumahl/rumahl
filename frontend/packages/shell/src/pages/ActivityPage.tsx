import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { EventMonitor } from "../shell/EventMonitor";

/** The shell's event monitor: the central activity log with filters. */
export function ActivityPage() {
  const { live } = useShell();
  const { t } = useI18n();
  return <section className="activity-page" aria-label={t("emergency.activityHeading")}>
    <h1>{t("emergency.activityHeading")}</h1>
    {live ? <EventMonitor request={live.request} /> : <p className="event-monitor__empty">{t("recovery.unavailable")}</p>}
  </section>;
}
