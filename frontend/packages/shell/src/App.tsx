import { useEffect, useMemo, useState, type PropsWithChildren } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { ThemeProvider } from "@rumahl/ui";
import { defineTheme, getTheme } from "@rumahl/ui/sdk";
import { defaultTheme, type Theme } from "@rumahl/ui/themes";
import { I18nProvider, useI18n } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { ShellRouter, type ShellRouterOptions } from "./routing/ShellRouter";
import { ShellPreferencesProvider, useShellPreferences } from "./preferences/ShellPreferences";
import { WorkspaceProvider } from "./preferences/Workspace";
import { AppCatalog } from "./apps/AppCatalog";
import { ShellLayout } from "./shell/ShellLayout";
import wallpaperUrl from "./assets/monstera.jpg";

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
  // Bridge the snapshot chrome into the theme model. Token values come from the
  // selected theme; the platform compiles the same tokens into the SSR stylesheet.
  const fallback = useMemo(() => defineTheme({
    id: "com.rumahl.shell",
    name: "rumahl",
    variants: { windowChrome: current.theme.windowChrome }
  }), [current.theme.windowChrome]);
  return <I18nProvider locale={current.user.locale}>
    <ShellRouter {...routing}>
      <ShellPreferencesProvider live={live}>
        <AppCatalog live={live} revision={current.revision}>
          <WorkspaceProvider live={live}>
            <ShellThemeGate override={theme} fallback={fallback}>
              {sessionExpired ? <SessionExpired /> : <ShellLayout snapshot={current} live={live} />}
            </ShellThemeGate>
          </WorkspaceProvider>
        </AppCatalog>
      </ShellPreferencesProvider>
    </ShellRouter>
  </I18nProvider>;
}

function ShellThemeGate({ override, fallback, children }: PropsWithChildren<{ override?: Theme | undefined; fallback: Theme }>) {
  const preferences = useShellPreferences();
  const theme = useMemo(() => {
    const selected = override ?? getTheme(preferences.theme) ?? defaultTheme;
    const merged = { ...selected, variants: { ...selected.variants, windowChrome: fallback.variants.windowChrome } };
    if (merged.tokens["texture.wallpaper"] !== "default") return merged;
    // `default` means the shell's bundled wallpaper; themes may override it.
    return { ...merged, tokens: { ...merged.tokens, "texture.wallpaper": `url(${wallpaperUrl})` } };
  }, [override, preferences.theme, fallback]);
  return <ThemeProvider theme={theme}>{children}</ThemeProvider>;
}

function SessionExpired() {
  const { t } = useI18n();
  return <main className="startup-message" role="status">{t("session.expired")} <a href="/login">{t("session.signIn")}</a></main>;
}
