import { useParams } from "react-router";
import { useI18n } from "../i18n";
export function UnavailablePage({ app = false }: { app?: boolean }) {
  const { t } = useI18n();
  const { appId } = useParams();
  return <main className="route-message" role="status">
    <h1>{t(app ? "route.appUnavailable" : "route.notFound")}</h1>
    {app ? <p>{t("route.appUnavailableBody", { id: appId ?? "" })}</p> : <p>{t("route.notFoundBody")}</p>}
  </main>;
}
