import { useEffect, useMemo, useRef, useState, type PropsWithChildren } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { ThemeProvider } from "@rumahl/ui";
import { WCAG, ensureContrast } from "@rumahl/ui/color";
import { type Theme } from "@rumahl/ui/themes";
import { I18nProvider } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { ShellRouter, type ShellRouterOptions } from "./routing/ShellRouter";
import { ShellPreferencesProvider, useShellPreferences } from "./preferences/ShellPreferences";
import { OsModeProvider } from "./preferences/OsMode";
import { applyThemeTuning, useThemeTuning } from "./preferences/theme-tuning";
import { WorkspaceProvider } from "./preferences/Workspace";
import { AppCatalog } from "./apps/AppCatalog";
import { ShellLayout } from "./shell/ShellLayout";
import { useSystemMode } from "./shell/system-scheme";
import { buildSnapshotTheme, resolveActiveTheme, resolveTokens } from "./theme";
import { rememberLoginAppearance } from "./preferences/login-appearance";
import { ShellFeatureProvider } from "./features/ShellFeatures";
import { useAdaptiveTypeScale } from "./preferences/adaptive-type";
import { flushSessionSnapshot } from "./shell/session-snapshot";

export function App({ snapshot, live, theme, ...routing }: ShellRouterOptions & {
  snapshot: ShellSnapshotV1;
  live?: ShellLiveSource | undefined;
  theme?: Theme | undefined;
}) {
  const [current, setCurrent] = useState(snapshot);
  const redirecting = useRef(false);
  useEffect(() => {
    if (!live) return;
    return watchShellUpdates(snapshot.revision, live, setCurrent, () => {
      if (redirecting.current) return;
      redirecting.current = true;
      const target = `${location.pathname}${location.search}`;
      const next = target && target !== "/" ? `/login?next=${encodeURIComponent(target)}` : "/login";
      const timeout = new Promise((resolve) => setTimeout(resolve, 600));
      void Promise.race([flushSessionSnapshot(), timeout]).finally(() => window.location.replace(next));
    }, (state) => {
      // The server only signals a real restart or power-off. Persist it so the
      // offline/recovery screen shows the right message, then hand over.
      try { localStorage.setItem("rumahl.lifecycle", JSON.stringify({ state, at: Date.now() })); } catch { /* ignore */ }
      location.assign(`/recovery?reason=${state === "shutting-down" ? "poweroff" : "restart"}`);
    });
  }, [live, snapshot.revision]);
  // The platform resolves the account theme and ships its tokens and variants.
  // Device overrides are resolved from the local theme registry.
  const serverTheme = useMemo(() => buildSnapshotTheme(current), [current]);
  return <I18nProvider locale={current.user.locale}>
    <ShellFeatureProvider demo={Boolean(live?.demo)}>
      <ShellRouter {...routing}>
        <ShellPreferencesProvider live={live} initial={{ mode: current.mode, theme: current.theme.id }}>
          <OsModeProvider live={live}>
            <AppCatalog live={live} revision={current.revision} initial={current.apps}>
              <WorkspaceProvider live={live} initial={current.workspace}>
                <ShellThemeGate override={theme} serverTheme={serverTheme}>
                  <ShellLayout snapshot={current} live={live} />
                </ShellThemeGate>
              </WorkspaceProvider>
            </AppCatalog>
          </OsModeProvider>
        </ShellPreferencesProvider>
      </ShellRouter>
    </ShellFeatureProvider>
  </I18nProvider>;
}

function ShellThemeGate({ override, serverTheme, children }: PropsWithChildren<{ override?: Theme | undefined; serverTheme: Theme }>) {
  const preferences = useShellPreferences();
  const { tuning } = useThemeTuning();
  const systemMode = useSystemMode();
  const adaptiveScale = useAdaptiveTypeScale(tuning.adaptiveType === true);
  const theme = useMemo(() => {
    const selected = override ?? resolveActiveTheme(serverTheme, preferences.theme);
    const tokens = { ...resolveTokens(applyThemeTuning(selected, tuning, systemMode)) };
    if (tuning.adaptiveType === true) tokens["typography.scale"] = String(adaptiveScale);
    return { ...selected, tokens };
  }, [override, preferences.theme, serverTheme, tuning, systemMode, adaptiveScale]);
  useEffect(() => {
    rememberLoginAppearance(theme, tuning.mode ?? systemMode);
  }, [theme, tuning.mode, systemMode]);
  useEffect(() => {
    const root = document.documentElement;
    const accent = theme.tokens["color.accent"];
    const panel = theme.tokens["color.panel.background"];
    if (!/^#[0-9a-f]{6}/i.test(accent) || !/^#[0-9a-f]{6}/i.test(panel)) return;
    root.style.setProperty("--rumahl-ui-color-accent-ink", ensureContrast(accent, panel, WCAG.AA));
    return () => { root.style.removeProperty("--rumahl-ui-color-accent-ink"); };
  }, [theme]);
  return <ThemeProvider theme={theme}>{children}</ThemeProvider>;
}
