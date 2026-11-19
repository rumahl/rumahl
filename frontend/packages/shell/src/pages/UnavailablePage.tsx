import { useParams } from "react-router";
import { useI18n } from "../i18n";
import { AppErrorCard } from "../components/AppErrorCard";
import { buttonClass } from "../components/Button";
import { ShellLink } from "../routing/ShellLink";

export function UnavailablePage({ app = false }: { app?: boolean }) {
  const { t } = useI18n();
  const { appId } = useParams();
  return <AppErrorCard
    title={t(app ? "route.appUnavailable" : "route.notFound")}
    message={app ? t("route.appUnavailableBody", { id: appId ?? "" }) : t("route.notFoundBody")}
    actions={app ? <ShellLink className={buttonClass("secondary")} to="/">{t("nav.home")}</ShellLink> : undefined}
  />;
}
