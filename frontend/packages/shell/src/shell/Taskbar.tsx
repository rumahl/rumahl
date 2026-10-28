import { windowBelongsToLink } from "./dock-model";
import { useState } from "react";
import { useLocation } from "react-router";
import { AppIcon } from "../apps/AppTile";
import { RumahlMark } from "../components/RumahlMark";
import { useShellApps } from "../apps/useShellApps";
import { GridIcon, HomeIcon, PulseIcon, SettingsIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { describeRoute } from "../routing/routes";
import { useShell } from "./ShellContext";
import { StartMenu } from "./launcher/StartMenu";

/**
 * Classic taskbar shell variant. It is structurally different from the floating
 * dock (full-width bar, labelled buttons, system tray) while still driven by the
 * same theme tokens, demonstrating that a theme can replace the shell layout.
 */
export function Taskbar() {
  const { t, locale } = useI18n();
  const apps = useShellApps();
  const { snapshot, state, dispatch, open, mode } = useShell();
  const location = useLocation();
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const [launcher, setLauncher] = useState(false);
  const windows = [...state.windows].sort((a, b) => a.id.localeCompare(b.id));
  const links = [
    { to: "/", label: t("nav.home"), icon: <HomeIcon /> },
    { to: "/apps", label: t("nav.apps"), icon: <GridIcon /> },
    { to: "/activity", label: t("nav.activity"), icon: <PulseIcon /> },
    { to: "/settings", label: t("nav.settings"), icon: <SettingsIcon /> }
  ];
  const extraWindows = windows.filter(item => !links.some(link => windowBelongsToLink(item, link.to)));
  const clock = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(new Date(snapshot.systemStatus.observedAtUnixMs));
  const windowIcon = (item: (typeof windows)[number]) => {
    const app = apps.find((candidate) => item.id === `app:${candidate.id}` || item.id === candidate.path || item.location === candidate.path);
    return app ? <AppIcon app={app} /> : <GridIcon />;
  };
  return <>
    <nav className="taskbar" aria-label={t("nav.main")}>
      <button className="taskbar__start" type="button" aria-label={mode === "desktop" ? t("launcher.open") : t("launcher.home")} aria-expanded={launcher}
        onClick={() => mode === "desktop" ? setLauncher((value) => !value) : open("/")}><RumahlMark /></button>
      <div className="taskbar__apps">
        {links.map(link => {
          const item = windows.find(window => windowBelongsToLink(window, link.to));
          const focused = item ? !item.minimized && state.focusedWindowId === item.id : active === describeRoute(link.to).id;
          return <ShellLink key={link.to} className="taskbar__button" to={item?.location ?? link.to} aria-label={link.label} title={link.label}
            data-open={!!item || (link.to === "/apps" && focused) || undefined} data-focused={focused || undefined}
            onClick={event => { if (item && !event.ctrlKey && !event.metaKey && !event.shiftKey && event.button === 0) { event.preventDefault(); dispatch({ type: "focus-window", id: item.id }); open(item.location ?? link.to); } }}>
            {link.icon}<span>{link.label}</span>
          </ShellLink>;
        })}
      </div>
      {extraWindows.length > 0 ? <div className="taskbar__windows" aria-label={t("desktop.running")}>
        {extraWindows.map((item) => <button key={item.id} type="button" className={`taskbar__button taskbar__window${item.minimized ? " is-minimized" : ""}`}
          aria-label={item.title} title={item.title} aria-pressed={!item.minimized && active === item.id}
          onClick={() => { if (!item.minimized && active === item.id && mode === "desktop") { dispatch({ type: "toggle-minimize", id: item.id }); open("/"); }
            else { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); } }}>
          <span aria-hidden="true">{windowIcon(item)}</span><span>{item.title}</span>
        </button>)}
      </div> : null}
      <div className="taskbar__tray">

        <time className="taskbar__clock" dateTime={new Date(snapshot.systemStatus.observedAtUnixMs).toISOString()}>{clock}</time>
      </div>
    </nav>
    {launcher ? <StartMenu onClose={() => setLauncher(false)} /> : null}
  </>;
}
