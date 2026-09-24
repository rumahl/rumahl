import { useEffect, useMemo, useReducer, useState } from "react";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { Dashboard } from "./components/Dashboard";
import { Navigation } from "./components/Navigation";
import { ProtectedWindow } from "./components/ProtectedWindow";
import { StreamRenderer } from "./components/StreamRenderer";
import { SectionPlaceholder } from "./components/SectionPlaceholder";
import { SearchIcon } from "./icons";
import { I18nProvider, resolveLocale, useI18n } from "./i18n";
import { watchShellUpdates, type ShellLiveSource } from "./live-updates";
import { initialShellState, shellReducer } from "./shell-state";
import { fetchStreamSessions, type StreamSession } from "./stream-client";

export function App({ snapshot, live }: { snapshot: ShellSnapshotV1; live?: ShellLiveSource | undefined }) {
  const [current, setCurrent] = useState(snapshot);
  const [sessionExpired, setSessionExpired] = useState(false);

  useEffect(() => {
    if (!live) return;
    return watchShellUpdates(snapshot.revision, live, setCurrent, () => setSessionExpired(true));
  }, [live, snapshot.revision]);

  return (
    <I18nProvider locale={current.user.locale}>
      {sessionExpired ? <SessionExpired /> : (
        <Shell
          allowDevelopmentLoopback={live?.allowDevelopmentLoopback}
          snapshot={current}
          widgetRequest={live?.request}
        />
      )}
    </I18nProvider>
  );
}

function SessionExpired() {
  const { t } = useI18n();
  return <main className="startup-message" role="status">{t("session.expired")} <a href="/login">{t("session.signIn")}</a></main>;
}

function Shell({ snapshot, widgetRequest, allowDevelopmentLoopback }: {
  snapshot: ShellSnapshotV1;
  widgetRequest?: ShellLiveSource["request"] | undefined;
  allowDevelopmentLoopback?: boolean | undefined;
}) {
  const [state, dispatch] = useReducer(shellReducer, initialShellState);
  const locale = resolveLocale(snapshot.user.locale);
  const { t } = useI18n();
  const [timeZone, setTimeZone] = useState("UTC");
  const [streamSessions, setStreamSessions] = useState<readonly StreamSession[]>([]);
  const [streamsUnavailable, setStreamsUnavailable] = useState(false);
  const managerOpen = state.windows.some((windowState) => windowState.id === "app-manager");
  const formatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", timeZone }),
    [locale, timeZone]
  );

  useEffect(() => {
    document.documentElement.lang = locale;
    setTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
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

  useEffect(() => {
    if (!managerOpen || !widgetRequest) return;
    let canceled = false;
    void fetchStreamSessions(widgetRequest).then(
      (sessions) => {
        if (!canceled) {
          setStreamSessions(sessions);
          setStreamsUnavailable(false);
        }
      },
      () => {
        if (!canceled) setStreamsUnavailable(true);
      }
    );
    return () => { canceled = true; };
  }, [managerOpen, snapshot.revision, widgetRequest]);

  const openAppManager = () =>
    dispatch({
      type: "open-window",
      window: {
        id: "app-manager",
        title: t("appManager.title"),
        subtitle: t("appManager.subtitle")
      }
    });

  const openStream = (session: StreamSession) =>
    dispatch({
      type: "open-window",
      window: {
        id: `stream:${session.id}`,
        title: session.title,
        subtitle: t("stream.subtitle"),
        streamId: session.id
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
            <form action="/logout" method="post"><button type="submit">{t("session.signOut")}</button></form>
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
            revision={snapshot.revision}
            onOpenApps={openAppManager}
            systemStatus={snapshot.systemStatus}
            widgetRequest={widgetRequest}
            allowDevelopmentLoopback={allowDevelopmentLoopback}
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
                stream={windowState.streamId !== undefined}
                title={windowState.title}
                variant={snapshot.theme.windowChrome}
              >
                {windowState.streamId ? (
                  <StreamRenderer
                    id={windowState.streamId}
                    request={widgetRequest}
                    title={windowState.title}
                  />
                ) : <div className="app-manager">
                  <div>
                    <p className="eyebrow">
                      {t("appManager.count", { count: snapshot.systemStatus.installedAppCount })}
                    </p>
                    <h3>{t("appManager.ready")}</h3>
                    <p>{t("appManager.body")}</p>
                    <h4>{t("stream.available")}</h4>
                    {streamsUnavailable ? <p role="status">{t("stream.unavailable")}</p> :
                      streamSessions.length === 0 ? <p>{t("stream.none")}</p> :
                      <div className="stream-list">
                        {streamSessions.map((session) => (
                          <button key={session.id} onClick={() => openStream(session)} type="button">
                            {session.title}
                          </button>
                        ))}
                      </div>}
                  </div>
                  <button type="button">{t("appManager.selectPackage")}</button>
                </div>}
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
