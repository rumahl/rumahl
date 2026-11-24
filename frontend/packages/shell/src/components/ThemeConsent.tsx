import { Button, RumahlModal } from "@rumahl/ui/components";
import type { Theme } from "@rumahl/ui/themes";
import { useI18n } from "../i18n";

export interface ThemeConsentProps {
  theme: Theme;
  codeHash: string | null;
  onCancel: () => void;
  onChoose: (execution: "sandbox" | "trusted") => void;
}

/**
 * Certificate-style consent for an executable theme. The safe choice is
 * sandboxed; full access is a deliberate, per-theme and per-code decision.
 */
export function ThemeConsent({ theme, codeHash, onCancel, onChoose }: ThemeConsentProps) {
  const { t } = useI18n();
  const runtime = theme.runtime ?? "shell";
  const capabilities = theme.capabilities?.length
    ? theme.capabilities.join(", ")
    : t("themes.noCapabilities");
  return <RumahlModal
    open
    kind="alert"
    onClose={onCancel}
    title={t("themes.codeTitle")}
    description={t("themes.codeDescription", { name: theme.name })}
    application="rumahl OS"
    closeLabel={t("themes.cancel")}
  >
    <div className="theme-consent">
      <dl>
        <div><dt>{t("themes.codeRuntime")}</dt><dd>{t(`themes.runtime.${runtime}`)}</dd></div>
        <div><dt>{t("themes.codeHash")}</dt><dd><code>{codeHash ?? "—"}</code></dd></div>
        <div><dt>{t("themes.codeCapabilities")}</dt><dd>{capabilities}</dd></div>
      </dl>
      <p className="theme-consent__warning">{t("themes.codeWarning")}</p>
      <p className="theme-consent__liability">{t("themes.codeLiability")}</p>
      <div className="theme-consent__actions">
        <Button variant="secondary" onClick={onCancel}>{t("themes.cancel")}</Button>
        {runtime !== "shell"
          ? <Button variant="primary" onClick={() => onChoose("sandbox")}>{t("themes.runSandboxed")}</Button>
          : <Button variant="secondary" onClick={() => onChoose("sandbox")}>{t("themes.installWithoutCode")}</Button>}
        {runtime !== "sandbox"
          ? <Button variant="danger" onClick={() => onChoose("trusted")}>{t("themes.allowFullAccess")}</Button>
          : null}
      </div>
    </div>
  </RumahlModal>;
}
