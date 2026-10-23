import { useI18n } from "../i18n";

/**
 * Mobile-only nudge shown when the desktop is requested in portrait. It keeps
 * the launcher usable and animates a phone turning into landscape so the next
 * step is obvious without reading a manual.
 */
export function MobileOrientationHint({ open, onDismiss }: { open: boolean; onDismiss: () => void }) {
  const { t } = useI18n();
  if (!open) return null;
  return <div className="orientation-hint" role="status" aria-live="polite">
    <span className="orientation-hint__phone" aria-hidden="true" />
    <span className="orientation-hint__copy">
      <strong>{t("orientation.title")}</strong>
      <span>{t("orientation.body")}</span>
    </span>
    <button className="orientation-hint__dismiss" type="button" onClick={onDismiss}>{t("orientation.dismiss")}</button>
  </div>;
}
