import { useLocation } from "react-router";
import { ProtectedWindow } from "../components/ProtectedWindow";
import { useI18n } from "../i18n";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { RouteBoundary } from "../routing/RouteBoundary";
import { useShell } from "./ShellContext";
export function DesktopWindows() {
  const { snapshot, state, dispatch, open } = useShell();
  const { t } = useI18n();
  const location = useLocation();
  const active = describeRoute(location.pathname).id;
  return <>
    <div aria-live="polite" className="window-layer">
      {state.windows.map((item) => item.minimized ? null :
        <div key={item.id} className="window-position">
          <ProtectedWindow id={item.id} focused={state.focusedWindowId === item.id}
            onClose={() => { dispatch({ type: "close-window", id: item.id }); if (active === item.id) open("/"); }}
            onFocus={() => { dispatch({ type: "focus-window", id: item.id }); if (active !== item.id && item.location) open(item.location); }}
            onMinimize={() => { dispatch({ type: "toggle-minimize", id: item.id }); if (active === item.id) open("/"); }}
            subtitle={item.subtitle} title={item.title} stream={item.streamId !== undefined} variant={snapshot.theme.windowChrome}>
            <RouteBoundary location={item.location ?? "/"}><ShellRoutes location={item.location ?? "/"} /></RouteBoundary>
          </ProtectedWindow>
        </div>
      )}
    </div>
    {state.windows.some((item) => item.minimized) ?
      <div aria-label={t("window.minimized")} className="window-dock">
        {state.windows.filter((item) => item.minimized).map((item) =>
          <button key={item.id} onClick={() => { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); }} type="button">{item.title}</button>
        )}
      </div> : null}
  </>;
}
