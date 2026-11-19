import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { EventMonitor } from "../shell/EventMonitor";
import { MobileNavBar } from "../components/MobileNavBar";
import { featureReady, useFeature } from "../features/ShellFeatures";

/** The shell's event monitor: the central activity log with filters. */
export function ActivityPage() {
  const { live, mode } = useShell();
  const activity = useFeature("activity");
  const { t } = useI18n();
  return <section className="activity-page" aria-label={t("emergency.activityHeading")}>
    {mode === "mobile" ? <MobileNavBar title={t("nav.activity")} /> : null}
    <h1>{t("emergency.activityHeading")}</h1>
    {!featureReady(activity) ? <p className="event-monitor__empty">{t("features.demoUnavailable")}</p>
      : live ? <EventMonitor request={live.request} /> : <p className="event-monitor__empty">{t("recovery.unavailable")}</p>}
  </section>;
}
