import { RumahlPageHeading } from "@rumahl/ui/components";
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
  return <RumahlPageHeading
    className="settings-page-heading"
    title={title}
    help={help}
    back={back ? <BackLink to={back} /> : null}
  />;
}
