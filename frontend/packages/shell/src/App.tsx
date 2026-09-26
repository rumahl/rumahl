import { useEffect, useState } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { I18nProvider, useI18n } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { ShellRouter, type ShellRouterOptions } from "./routing/ShellRouter";
import { ShellPreferencesProvider } from "./preferences/ShellPreferences";
import { AppCatalog } from "./apps/AppCatalog";
import { ShellLayout } from "./shell/ShellLayout";

export function App({ snapshot, live, ...routing }: ShellRouterOptions & {
  snapshot: ShellSnapshotV1;
  live?: ShellLiveSource | undefined;
}) {
  const [current, setCurrent] = useState(snapshot);
  const [sessionExpired, setSessionExpired] = useState(false);
  useEffect(() => {
    if (!live) return;
    return watchShellUpdates(snapshot.revision, live, setCurrent, () => setSessionExpired(true));
  }, [live, snapshot.revision]);
  return <I18nProvider locale={current.user.locale}>
    {sessionExpired ? <SessionExpired /> :
      <ShellRouter {...routing}><ShellPreferencesProvider live={live}><AppCatalog live={live} revision={current.revision}><ShellLayout snapshot={current} live={live} /></AppCatalog></ShellPreferencesProvider></ShellRouter>}
  </I18nProvider>;
}
function SessionExpired() {
  const { t } = useI18n();
  return <main className="startup-message" role="status">{t("session.expired")} <a href="/login">{t("session.signIn")}</a></main>;
}
