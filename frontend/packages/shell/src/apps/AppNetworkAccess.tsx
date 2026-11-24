import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { Button } from "../components/Button";
import { LoadingStatus } from "../components/LoadingStatus";
import {
  fetchNetworkOrigins, revokeNetworkOrigin, type AppNetworkOrigin, type InstalledApp
} from "./client";

/**
 * Network egress consents for one installed app. The app can ask for an
 * origin at runtime; this is the user's durable overview and the revocation
 * surface. Revocation takes effect on the next brokered request.
 */
export function AppNetworkAccess({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t, locale } = useI18n();
  const [origins, setOrigins] = useState<readonly AppNetworkOrigin[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [revoking, setRevoking] = useState<string | null>(null);

  const load = (signal: AbortSignal): void => {
    if (!live) return;
    fetchNetworkOrigins(live.request, app, signal)
      .then((value) => { if (!signal.aborted) setOrigins(value); })
      .catch(() => { if (!signal.aborted) setFailed(true); });
  };

  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setOrigins(null);
    setFailed(false);
    load(controller.signal);
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);

  const revoke = async (origin: string): Promise<void> => {
    if (!live || revoking !== null) return;
    setRevoking(origin);
    try {
      await revokeNetworkOrigin(live.request, app, origin, new AbortController().signal);
      const next = await fetchNetworkOrigins(live.request, app, new AbortController().signal);
      setOrigins(next);
    } catch {
      setFailed(true);
    } finally {
      setRevoking(null);
    }
  };

  return <section aria-labelledby="app-network">
    <h2 id="app-network">{t("appNetwork.title")}</h2>
    {failed
      ? <p role="alert">{t("appNetwork.unavailable")}</p>
      : !origins
        ? <LoadingStatus label={t("apps.loading")} />
        : origins.length === 0
          ? <p>{t("appNetwork.empty")}</p>
          : <ul className="app-data-list">{origins.map((origin) => <li key={origin.origin}>
              <span>
                <code>{origin.origin}</code>
                {" · "}
                {t(origin.validity === "session" ? "appNetwork.session" : "appNetwork.durable")}
                {" · "}
                {t(origin.allowPrivate ? "appNetwork.private" : "appNetwork.public")}
                {origin.expiresAt !== null
                  ? ` · ${t("appNetwork.expires", { date: new Intl.DateTimeFormat(locale).format(new Date(origin.expiresAt * 1000)) })}`
                  : ""}
              </span>
              <Button
                variant="ghost"
                type="button"
                disabled={revoking !== null}
                onClick={() => void revoke(origin.origin)}
              >{t("appNetwork.revoke")}</Button>
            </li>)}</ul>}
  </section>;
}
