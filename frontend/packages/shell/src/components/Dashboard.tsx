import type { ExtensionContribution, ShellSystemStatus } from "@rumahl/contracts";
import { ArrowIcon, GridIcon, PulseIcon, ShieldIcon } from "../icons";
import { useI18n } from "../i18n";

interface DashboardProps {
  contributions: readonly ExtensionContribution[];
  displayName: string;
  onOpenApps: () => void;
  systemStatus: ShellSystemStatus;
}

export function Dashboard({
  contributions,
  displayName,
  onOpenApps,
  systemStatus
}: DashboardProps) {
  const { formatRelativeTime, t } = useI18n();
  const commands = contributions.filter((item) => item.kind === "command");
  const widgets = contributions.filter((item) => item.kind === "widget");
  const protectionStatus =
    systemStatus.protection === "active"
      ? t("status.protection.value")
      : t("status.protection.attention");

  return (
    <main className="dashboard">
      <header className="dashboard__hero">
        <p className="eyebrow">{t("dashboard.greeting", { name: displayName })}</p>
        <h1>{t("dashboard.title")}</h1>
        <p className="dashboard__intro">{t("dashboard.intro")}</p>
      </header>

      <section aria-label={t("status.title")} className="status-grid">
        <article className="status-card status-card--accent">
          <div className="status-card__icon">
            <ShieldIcon />
          </div>
          <div>
            <p>{t("status.protection.label")}</p>
            <strong>{protectionStatus}</strong>
          </div>
          <span className="status-card__badge">
            {systemStatus.protection === "active"
              ? t("status.protection.active")
              : t("status.protection.attention")}
          </span>
        </article>
        <article className="status-card">
          <div className="status-card__icon">
            <GridIcon />
          </div>
          <div>
            <p>{t("status.apps.label")}</p>
            <strong>{t("status.apps.value", { count: systemStatus.installedAppCount })}</strong>
          </div>
          <button aria-label={t("status.apps.open")} onClick={onOpenApps} type="button">
            <ArrowIcon />
          </button>
        </article>
        <article className="status-card">
          <div className="status-card__icon">
            <PulseIcon />
          </div>
          <div>
            <p>{t("status.activity.label")}</p>
            <strong>
              {systemStatus.lastActivityAtUnixMs === null
                ? t("status.activity.none")
                : formatRelativeTime(
                    systemStatus.lastActivityAtUnixMs,
                    systemStatus.observedAtUnixMs
                  )}
            </strong>
          </div>
        </article>
      </section>

      <section className="dashboard__section">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t("contribution.source")}</p>
            <h2>{t("contribution.title")}</h2>
          </div>
          <span>{t("contribution.count", { count: commands.length + widgets.length })}</span>
        </div>
        <div className="contribution-grid">
          {commands.map((command) => (
            <button className="contribution-card" key={command.id} type="button">
              <span className="contribution-card__monogram">+</span>
              <span>
                <strong>{command.title}</strong>
                <small>{t("contribution.protected")}</small>
              </span>
              <ArrowIcon />
            </button>
          ))}
          {widgets.map((widget) => (
            <article className="contribution-card contribution-card--widget" key={widget.id}>
              <span className="contribution-card__monogram">⌂</span>
              <span>
                <strong>{widget.title}</strong>
                <small>{t("contribution.widget")}</small>
              </span>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
