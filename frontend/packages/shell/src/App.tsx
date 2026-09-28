import { useEffect, useMemo, useState } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { ThemeProvider } from "@rumahl/ui";
import { defineTheme } from "@rumahl/ui/sdk";
import type { Theme } from "@rumahl/ui/themes";
import { I18nProvider, useI18n } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { ShellRouter, type ShellRouterOptions } from "./routing/ShellRouter";
import { ShellPreferencesProvider } from "./preferences/ShellPreferences";
import { WorkspaceProvider } from "./preferences/Workspace";
import { AppCatalog } from "./apps/AppCatalog";
import { ShellLayout } from "./shell/ShellLayout";

export function App({ snapshot, live, theme, ...routing }: ShellRouterOptions & {
  snapshot: ShellSnapshotV1;
  live?: ShellLiveSource | undefined;
  theme?: Theme | undefined;
}) {
  const [current, setCurrent] = useState(snapshot);
  const [sessionExpired, setSessionExpired] = useState(false);
  useEffect(() => {
    if (!live) return;
    return watchShellUpdates(snapshot.revision, live, setCurrent, () => setSessionExpired(true));
  }, [live, snapshot.revision]);
  // Bridge the current snapshot chrome into the theme model. Token values stay
  // at the rumahl defaults until the platform ships theme tokens over the wire.
  const derivedTheme = useMemo(() => defineTheme({
    id: "com.rumahl.shell",
    name: "rumahl",
    variants: { windowChrome: current.theme.windowChrome }
  }), [current.theme.windowChrome]);
  const activeTheme = theme ?? derivedTheme;
  return <I18nProvider locale={current.user.locale}>
    <ThemeProvider theme={activeTheme}>
      {sessionExpired ? <SessionExpired /> :
        <ShellRouter {...routing}><ShellPreferencesProvider live={live}><AppCatalog live={live} revision={current.revision}><WorkspaceProvider live={live}><ShellLayout snapshot={current} live={live} /></WorkspaceProvider></AppCatalog></ShellPreferencesProvider></ShellRouter>}
    </ThemeProvider>
  </I18nProvider>;
}
function SessionExpired() {
  const { t } = useI18n();
  return <main className="startup-message" role="status">{t("session.expired")} <a href="/login">{t("session.signIn")}</a></main>;
}
