import { windowBelongsToLink } from "./dock-model";
import { AppIcon } from "../apps/AppTile";
import { isLaunching, useLaunching } from "../apps/launching";
import { RumahlMark } from "../components/RumahlMark";
import { useShellApps } from "../apps/useShellApps";
import { useRef, useState } from "react";
import { useLocation } from "react-router";
import { HomeIcon, GridIcon, SettingsIcon, PulseIcon } from "../icons";
import { ShellLink } from "../routing/ShellLink";
import { describeRoute } from "../routing/routes";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";
import { StartMenu } from "./launcher/StartMenu";
import { useGlassHost } from "../glass-engine/useGlassEngine";
import { GLASS_MATERIALS } from "@rumahl/ui/glass";

export function OsDock() {
  const dockRef = useRef<HTMLElement>(null);
  useGlassHost(dockRef, GLASS_MATERIALS.dock);
  const { t } = useI18n();
  const apps = useShellApps();
  const { state, dispatch, open, mode } = useShell();
  useLaunching();
  const location = useLocation();
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const [launcher, setLauncher] = useState(false);
  const [hover, setHover] = useState<string | null>(null);
  const windows = [...state.windows].sort((a, b) => a.id.localeCompare(b.id));
  const links = [
    { to: "/", id: "home", label: t("nav.home"), icon: <HomeIcon /> },
    { to: "/apps", id: "apps", label: t("nav.apps"), icon: <GridIcon /> },
    { to: "/activity", id: "activity", label: t("nav.activity"), icon: <PulseIcon /> },
    { to: "/settings", id: "settings", label: t("nav.settings"), icon: <SettingsIcon /> }
  ];
  const extraWindows = windows.filter(item => !links.some(link => windowBelongsToLink(item, link.to)));
  const order = ["start", ...links.map(link => link.to), ...extraWindows.map(item => item.id)];
  const activate = (item: (typeof windows)[number]) => {
    if (!item.minimized && state.focusedWindowId === item.id && mode === "desktop") {
      dispatch({ type: "toggle-minimize", id: item.id }); open("/");
    } else { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); }
  };
  const magnify = (id: string) => {
    if (hover === id) return " is-hovered";
    if (hover && Math.abs(order.indexOf(hover) - order.indexOf(id)) === 1) return " is-neighbor";
    return "";
  };
  return <>
    <nav ref={dockRef} className={`dock os-dock rumahl-glass-host os-dock--${mode}`} aria-label={t("nav.main")} onPointerLeave={() => setHover(null)}>
      <button className={`dock-item dock-icon dock-start${magnify("start")}`} type="button" aria-label={mode === "desktop" ? t("launcher.open") : t("launcher.home")} aria-expanded={launcher}
        onPointerEnter={() => setHover("start")}
        onClick={() => mode === "desktop" ? setLauncher((value) => !value) : open("/")}><RumahlMark /></button>
      {links.map(link => {
        const item = windows.find(window => windowBelongsToLink(window, link.to));
        const focused = item ? !item.minimized && state.focusedWindowId === item.id : active === describeRoute(link.to).id;
        const running = !!item || (link.to === "/apps" && focused);
        return <ShellLink key={link.to} className={`dock-item dock-icon${magnify(link.to)}${running ? " is-running" : ""}${item?.minimized ? " is-minimized" : ""}`}
          to={item?.location ?? link.to} aria-label={link.label} title={link.label} data-dock-app={link.id}
          data-open={running || undefined} data-focused={focused || undefined} aria-current={focused ? "page" : undefined}
          onClick={event => { if (item && !event.ctrlKey && !event.metaKey && !event.shiftKey && event.button === 0) { event.preventDefault(); activate(item); } }}
          onPointerEnter={() => setHover(link.to)}>{link.icon}{running ? <i className="dock-dot" aria-hidden="true" /> : null}</ShellLink>;
      })}
      {extraWindows.length > 0 ? <span className="dock-separator" aria-hidden="true" /> : null}
      {extraWindows.length > 0 ? <div className="dock-windows" aria-label={t("desktop.running")}>
        {extraWindows.map((item) => {
          const appId = item.id.startsWith("app:") ? item.id.slice("app:".length) : undefined;
          const bouncing = appId !== undefined && isLaunching(appId);
          return <button key={item.id} type="button" aria-label={item.title} title={item.title}
            data-open="true" data-focused={!item.minimized && state.focusedWindowId === item.id || undefined} aria-pressed={!item.minimized && state.focusedWindowId === item.id} className={`dock-item dock-icon${magnify(item.id)}${item.minimized ? " is-minimized" : " is-running"}${bouncing ? " is-launching" : ""}`}
            onPointerEnter={() => setHover(item.id)}
            onClick={() => activate(item)}><span aria-hidden="true">{(() => { const app = apps.find(app => item.id === `app:${app.id}` || item.id === app.path || item.location === app.path); const systemLink = links.find(link => item.location === link.to || (link.to !== "/" && item.location?.startsWith(`${link.to}/`))); return app ? <AppIcon app={app}/> : systemLink?.icon ?? <GridIcon/>; })()}</span><i className={`dock-dot${item.minimized ? " is-minimized" : ""}`} aria-hidden="true" /></button>;
        })}
      </div> : null}
    </nav>
    {launcher ? <StartMenu onClose={() => setLauncher(false)} /> : null}
  </>;
}
