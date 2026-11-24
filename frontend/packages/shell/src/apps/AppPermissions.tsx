import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { LoadingStatus } from "../components/LoadingStatus";
import { Button } from "../components/Button";
import {
  fetchAppGrantState, revokeAppGrant,
  type AppGrantInfo, type AppGrantState, type AppPermissionRequestInfo, type InstalledApp
} from "./client";

/**
 * The consent state of one installed app: which permissions its manifest
 * requests, and which grants the user has issued. The user can revoke any
 * grant at any time; revocation takes effect on the next app operation.
 */
export function AppPermissions({ app }: { app: InstalledApp }) {
  const { live } = useShell();
  const { t } = useI18n();
  const [state, setState] = useState<AppGrantState | null>(null);
  const [failed, setFailed] = useState(false);
  const [revoking, setRevoking] = useState<string | null>(null);

  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setState(null);
    setFailed(false);
    fetchAppGrantState(live.request, app, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setState(value); })
      .catch(() => { if (!controller.signal.aborted) setFailed(true); });
    return () => controller.abort();
  }, [live, app.id, app.installationId, app.version]);

  const revoke = async (grant: AppGrantInfo) => {
    if (!live || revoking !== null) return;
    setRevoking(grant.id);
    try {
      await revokeAppGrant(live.request, app, grant.id, new AbortController().signal);
      const next = await fetchAppGrantState(live.request, app, new AbortController().signal);
      setState(next);
    } catch {
      setFailed(true);
    } finally {
      setRevoking(null);
    }
  };

  return <section aria-labelledby="app-permissions" className="app-permissions">
    <h2 id="app-permissions">{t("appPermissions.title")}</h2>
    {failed
      ? <p role="alert">{t("appPermissions.unavailable")}</p>
      : !state
        ? <LoadingStatus label={t("apps.loading")} />
        : <>
            <h3>{t("appPermissions.requests")}</h3>
            {state.requests.length === 0
              ? <p>{t("appPermissions.requestedNone")}</p>
              : <dl>{state.requests.map((request) =>
                  <RequestedPermission key={request.permission} request={request} />)}</dl>}
            <h3>{t("appPermissions.grants")}</h3>
            {state.grants.length === 0
              ? <p>{t("appPermissions.grantsNone")}</p>
              : <ul className="app-data-list">{state.grants.map((grant) =>
                  <li key={grant.id} className="app-permissions__grant">
                    <span>
                      {grant.permission}
                      {" · "}
                      {t(grant.validity === "session" ? "appPermissions.session" : "appPermissions.durable")}
                      {grant.expiresAt !== null
                        ? ` · ${t("appPermissions.expires")}: ${new Intl.DateTimeFormat().format(new Date(grant.expiresAt * 1000))}`
                        : ""}
                      {" · "}
                      {t("appPermissions.resources", { count: grant.resources.length })}
                    </span>
                    <Button
                      className="app-permissions__revoke"
                      variant="ghost"
                      type="button"
                      disabled={revoking !== null}
                      onClick={() => void revoke(grant)}
                    >
                      {revoking === grant.id ? t("appPermissions.revoking") : t("appPermissions.revoke")}
                    </Button>
                  </li>)}</ul>}
          </>}
  </section>;
}

function RequestedPermission({ request }: { request: AppPermissionRequestInfo }) {
  const { t } = useI18n();
  return <div className="app-setting" data-permission={request.permission}>
    <dt>
      {request.permission}
      {request.required ? <span className="app-setting__required">{t("appPermissions.required")}</span> : null}
      {" · "}
      <span className="app-setting__type">{t("appPermissions.scope")}: {request.scope}</span>
    </dt>
    {request.reason ? <dd className="app-setting__description">{request.reason}</dd> : null}
  </div>;
}
