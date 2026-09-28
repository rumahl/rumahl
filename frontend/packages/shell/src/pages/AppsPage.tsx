import { AppGrid } from "../apps/AppGrid";
import { useI18n } from "../i18n";
export function AppsPage() {
  const { t } = useI18n();
  return <main className="app-catalog"><h1>{t("nav.apps")}</h1><AppGrid /></main>;
}
