import { useEffect, useMemo, useState, type PropsWithChildren } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { ThemeProvider } from "@rumahl/ui";
import { type Theme } from "@rumahl/ui/themes";
import { I18nProvider, useI18n } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { ShellRouter, type ShellRouterOptions } from "./routing/ShellRouter";
import { ShellPreferencesProvider, useShellPreferences } from "./preferences/ShellPreferences";
import { applyTuning, useThemeTuning } from "./preferences/theme-tuning";
import { WorkspaceProvider } from "./preferences/Workspace";
import { AppCatalog } from "./apps/AppCatalog";
import { ShellLayout } from "./shell/ShellLayout";
import { useSystemMode } from "./shell/system-scheme";
import { buildSnapshotTheme, resolveActiveTheme, resolveTokens } from "./theme";

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
  // The platform resolves the account theme and ships its tokens and variants.
  // Device overrides are resolved from the local theme registry.
  const serverTheme = useMemo(() => buildSnapshotTheme(current), [current]);
  return <I18nProvider locale={current.user.locale}>
    <ShellRouter {...routing}>
      <ShellPreferencesProvider live={live} initial={{ mode: current.mode, theme: current.theme.id }}>
        <AppCatalog live={live} revision={current.revision} initial={current.apps}>
          <WorkspaceProvider live={live} initial={current.workspace}>
            <ShellThemeGate override={theme} serverTheme={serverTheme}>
              {sessionExpired ? <SessionExpired /> : <ShellLayout snapshot={current} live={live} />}
            </ShellThemeGate>
          </WorkspaceProvider>
        </AppCatalog>
      </ShellPreferencesProvider>
    </ShellRouter>
  </I18nProvider>;
}

function ShellThemeGate({ override, serverTheme, children }: PropsWithChildren<{ override?: Theme | undefined; serverTheme: Theme }>) {
  const preferences = useShellPreferences();
  const { tuning } = useThemeTuning();
  const systemMode = useSystemMode();
  const theme = useMemo(() => {
    const selected = override ?? resolveActiveTheme(serverTheme, preferences.theme);
    return { ...selected, tokens: resolveTokens(applyTuning(selected.tokens, tuning, systemMode)) };
  }, [override, preferences.theme, serverTheme, tuning, systemMode]);
  return <ThemeProvider theme={theme}>{children}</ThemeProvider>;
}

function SessionExpired() {
  const { t } = useI18n();
  return <main className="startup-message" role="status">{t("session.expired")} <a href="/login">{t("session.signIn")}</a></main>;
}
