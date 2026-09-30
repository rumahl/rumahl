import { ArrowLeftIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";

/** Back affordance shown before the title on settings sub-pages. */
export function BackLink({ to }: { to: string }) {
  const { t } = useI18n();
  return <ShellLink className="settings-back" to={to} aria-label={t("settings.back")} title={t("settings.back")}>
    <ArrowLeftIcon />
  </ShellLink>;
}

/** Settings page header with an optional back button before the title. */
export function SettingsHeading({ title, help, back }: { title: string; help?: string; back?: string }) {
  return <header className="settings-page-heading">
    <div className="settings-heading">
      {back ? <BackLink to={back} /> : null}
      <h1 className="pagetitle">{title}</h1>
    </div>
    {help ? <p className="pagedesc">{help}</p> : null}
  </header>;
}
