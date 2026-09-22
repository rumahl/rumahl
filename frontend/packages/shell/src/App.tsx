import { useEffect, useMemo, useReducer } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { Dashboard } from "./components/Dashboard";
import { Navigation } from "./components/Navigation";
import { ProtectedWindow } from "./components/ProtectedWindow";
import { SectionPlaceholder } from "./components/SectionPlaceholder";
import { SearchIcon } from "./icons";
import { I18nProvider, resolveLocale, useI18n } from "./i18n";
import { initialShellState, shellReducer } from "./shell-state";

export function App({ snapshot }: { snapshot: ShellSnapshotV1 }) {
  return (
    <I18nProvider locale={snapshot.user.locale}>
      <Shell snapshot={snapshot} />
    </I18nProvider>
  );
}

function Shell({ snapshot }: { snapshot: ShellSnapshotV1 }) {
  const [state, dispatch] = useReducer(shellReducer, initialShellState);
  const locale = resolveLocale(snapshot.user.locale);
  const { t } = useI18n();
  const formatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }),
    [locale]
  );

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        dispatch({ type: "toggle-command-palette" });
      }
      if (event.key === "Escape" && state.commandPaletteOpen) {
        dispatch({ type: "toggle-command-palette" });
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state.commandPaletteOpen]);

  const openAppManager = () =>
    dispatch({
      type: "open-window",
      window: {
        id: "app-manager",
        title: t("appManager.title"),
        subtitle: t("appManager.subtitle")
      }
    });

  return (
    <div className="shell" data-shell-build={snapshot.shellBuildId}>
      <Navigation
        active={state.section}
        onNavigate={(section) => dispatch({ type: "navigate", section })}
      />
      <div className="shell__workspace">
        <header className="topbar">
          <button
            aria-expanded={state.commandPaletteOpen}
            className="search-trigger"
            onClick={() => dispatch({ type: "toggle-command-palette" })}
            type="button"
          >
            <SearchIcon />
            <span>{t("search.system")}</span>
            <kbd>⌘ K</kbd>
          </button>
          <div className="topbar__profile">
            <span>{formatter.format(new Date(snapshot.systemStatus.observedAtUnixMs))}</span>
            <button aria-label={t("profile.open")} type="button">
              {snapshot.user.displayName.slice(0, 1).toUpperCase()}
            </button>
          </div>
        </header>

        {state.section === "home" ? (
          <Dashboard
            contributions={snapshot.contributions}
            displayName={snapshot.user.displayName}
            onOpenApps={openAppManager}
            systemStatus={snapshot.systemStatus}
          />
        ) : (
          <SectionPlaceholder section={state.section} />
        )}

        <div aria-live="polite" className="window-layer">
          {state.windows.map((windowState) =>
            windowState.minimized ? null : (
              <ProtectedWindow
                focused={state.focusedWindowId === windowState.id}
                id={windowState.id}
                key={windowState.id}
                onClose={() => dispatch({ type: "close-window", id: windowState.id })}
                onFocus={() => dispatch({ type: "focus-window", id: windowState.id })}
                onMinimize={() => dispatch({ type: "toggle-minimize", id: windowState.id })}
                subtitle={windowState.subtitle}
                title={windowState.title}
                variant={snapshot.theme.windowChrome}
              >
                <div className="app-manager">
                  <div>
                    <p className="eyebrow">
                      {t("appManager.count", { count: snapshot.systemStatus.installedAppCount })}
                    </p>
                    <h3>{t("appManager.ready")}</h3>
                    <p>{t("appManager.body")}</p>
                  </div>
                  <button type="button">{t("appManager.selectPackage")}</button>
                </div>
              </ProtectedWindow>
            )
          )}
        </div>

        {state.windows.some((windowState) => windowState.minimized) ? (
          <div aria-label={t("window.minimized")} className="window-dock">
            {state.windows
              .filter((windowState) => windowState.minimized)
              .map((windowState) => (
                <button
                  key={windowState.id}
                  onClick={() => dispatch({ type: "focus-window", id: windowState.id })}
                  type="button"
                >
                  {windowState.title}
                </button>
              ))}
          </div>
        ) : null}

        {state.commandPaletteOpen ? (
          <div className="command-backdrop" role="presentation">
            <section aria-label={t("command.title")} className="command-palette" role="dialog">
              <div className="command-palette__input">
                <SearchIcon />
                <input
                  aria-label={t("command.input")}
                  autoFocus
                  placeholder={t("command.placeholder")}
                />
                <kbd>Esc</kbd>
              </div>
              <div className="command-palette__results">
                <p>{t("command.available")}</p>
                {snapshot.contributions
                  .filter((item) => item.kind === "command")
                  .map((command) => (
                    <button key={command.id} type="button">
                      <span>{command.title}</span>
                      <small>{command.capability}</small>
                    </button>
                  ))}
              </div>
            </section>
          </div>
        ) : null}
      </div>
    </div>
  );
}
