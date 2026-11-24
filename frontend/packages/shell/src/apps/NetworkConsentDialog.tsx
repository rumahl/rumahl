import { useState } from "react";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { RumahlModal } from "../components/RumahlDialog";
import { Button } from "../components/Button";
import { grantNetworkOrigin, type InstalledApp } from "./client";

/** One pending network consent from an installed app. */
export interface NetworkConsentRequest {
  origin: string;
  resolve: (granted: boolean) => void;
}

/**
 * The consent surface for app network egress. The app names a URL; the user
 * sees the exact origin and chooses a session or durable approval. The broker
 * re-checks the stored consent on every request, so this dialog is a decision,
 * not a one-time formality.
 */
export function NetworkConsentDialog({ app, request, onDone }: {
  app: InstalledApp;
  request: NetworkConsentRequest;
  onDone: (granted: boolean) => void;
}) {
  const { live } = useShell();
  const { t } = useI18n();
  const [busy, setBusy] = useState<"session" | "durable" | null>(null);
  const [failed, setFailed] = useState(false);

  const allow = async (validity: "session" | "durable") => {
    if (!live || busy) return;
    setBusy(validity);
    setFailed(false);
    try {
      await grantNetworkOrigin(
        live.request, app, request.origin, validity, new AbortController().signal
      );
      onDone(true);
    } catch {
      setFailed(true);
      setBusy(null);
    }
  };

  return <RumahlModal
    open
    onClose={() => onDone(false)}
    title={t("netBridge.title")}
    application={app.title}
    closeLabel={t("netBridge.close")}
    description={t("netBridge.help", { title: app.title })}
    actions={<>
      <Button variant="ghost" type="button" disabled={busy !== null} onClick={() => onDone(false)}>
        {t("netBridge.decline")}
      </Button>
      <Button variant="secondary" type="button" disabled={busy !== null} onClick={() => void allow("session")}>
        {t("netBridge.session")}
      </Button>
      <Button variant="primary" type="button" disabled={busy !== null} onClick={() => void allow("durable")}>
        {t("netBridge.always")}
      </Button>
    </>}
  >
    <dl className="net-consent">
      <dt>{t("netBridge.origin")}</dt>
      <dd><code>{request.origin}</code></dd>
      <dt>{t("netBridge.app")}</dt>
      <dd>{app.title}</dd>
    </dl>
    <p>{t("netBridge.warning")}</p>
    {failed ? <p role="alert">{t("netBridge.failed")}</p> : null}
  </RumahlModal>;
}
