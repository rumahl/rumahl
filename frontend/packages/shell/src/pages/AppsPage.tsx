import { useAppCatalog } from "../apps/AppCatalog";
import { firstPartyApps } from "../apps/registry";
import { useI18n } from "../i18n";
import { appPath } from "../routing/paths";
import { ShellLink } from "../routing/ShellLink";
export function AppsPage() {
  const { t } = useI18n();
  const catalog = useAppCatalog();
  return <main className="app-catalog"><h1>{t("nav.apps")}</h1>
    <div className="app-grid">{firstPartyApps.map((app) =>
      <ShellLink key={app.id} to={appPath(app.id)}>{t(app.title)}</ShellLink>
    )}</div>
    <h2>{t("apps.installed")}</h2>
    {catalog.status === "loading" ? <p role="status">{t("apps.loading")}</p> : catalog.status === "unavailable" ? <p role="status">{t("apps.catalogUnavailable")}</p> :
      catalog.apps.length === 0 ? <p>{t("apps.empty")}</p> :
        <div className="app-grid">{catalog.apps.filter((app) => !firstPartyApps.some((system) => system.id === app.id)).map((app) =>
          <ShellLink key={app.installationId} to={appPath(app.id)}>{app.title}{!app.launchable ? <small> — {t("apps.notLaunchable")}</small> : null}</ShellLink>
        )}</div>}

  </main>;
}
