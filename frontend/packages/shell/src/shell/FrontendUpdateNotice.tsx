import { useI18n } from "../i18n";
import { applyFrontendUpdate, dismissFrontendUpdate, useFrontendUpdate } from "../frontend-updates";

/**
 * Dock notification for frontend updates. It sits directly above the dock: the
 * download state is announced without blocking the shell, and only a completed
 * package offers the reload action.
 */
export function FrontendUpdateNotice() {
  const { t } = useI18n();
  const update = useFrontendUpdate();
  if (update.phase === "idle") return null;
  const downloading = update.phase === "downloading";
  return <section className="frontend-update" data-phase={update.phase} role="status" aria-live="polite">
    <span className="frontend-update__icon" aria-hidden="true" />
    <span className="frontend-update__copy">
      <strong>{t(downloading ? "update.downloadingTitle" : "update.readyTitle")}</strong>
      <span>{t(downloading ? "update.downloadingBody" : "update.readyBody")}</span>
      {downloading ? <span className="frontend-update__bar" aria-hidden="true"><i /></span> : null}
    </span>
    {downloading ? null : <span className="frontend-update__actions">
      <button className="frontend-update__primary" type="button" onClick={applyFrontendUpdate}>{t("reload.now")}</button>
      <button type="button" onClick={dismissFrontendUpdate}>{t("reload.later")}</button>
    </span>}
  </section>;
}
