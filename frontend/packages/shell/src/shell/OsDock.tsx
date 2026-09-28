import { AppIcon } from "../apps/AppTile";
import { RumahlMark } from "../components/RumahlMark";
import { useShellApps } from "../apps/useShellApps";
import { useState } from "react";
import { useLocation } from "react-router";
import { HomeIcon, GridIcon, SettingsIcon, PulseIcon } from "../icons";
import { ShellLink } from "../routing/ShellLink";
import { describeRoute } from "../routing/routes";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";
import { StartMenu } from "./launcher/StartMenu";
export function OsDock() {
  const { t } = useI18n();
  const apps = useShellApps();
  const { state, dispatch, open, mode } = useShell();
  const location = useLocation();
  const active = describeRoute(location.pathname).id;
  const [launcher, setLauncher] = useState(false);
  const [hover, setHover] = useState<string | null>(null);
  const windows = [...state.windows].sort((a, b) => a.id.localeCompare(b.id));
  const links = [
    { to: "/", id: "home", label: t("nav.home"), icon: <HomeIcon /> },
    { to: "/apps", id: "apps", label: t("nav.apps"), icon: <GridIcon /> },
    { to: "/activity", id: "activity", label: t("nav.activity"), icon: <PulseIcon /> },
    { to: "/settings", id: "settings", label: t("nav.settings"), icon: <SettingsIcon /> }
  ];
  const order = ["start", ...links.map((link) => link.to), ...windows.map((item) => item.id), "desktop"];
  const magnify = (id: string) => {
    if (hover === id) return " is-hovered";
    if (hover && Math.abs(order.indexOf(hover) - order.indexOf(id)) === 1) return " is-neighbor";
    return "";
  };
  return <>
    <nav className={`os-dock os-dock--${mode}`} aria-label={t("nav.main")} onPointerLeave={() => setHover(null)}>
      <button className={`dock-item dock-start${magnify("start")}`} type="button" aria-label={mode === "desktop" ? t("launcher.open") : t("launcher.home")} aria-expanded={launcher}
        onPointerEnter={() => setHover("start")}
        onClick={() => mode === "desktop" ? setLauncher((value) => !value) : open("/")}><RumahlMark /></button>
      {links.map((link) => <ShellLink key={link.to} className={`dock-item${magnify(link.to)}`} to={link.to} aria-label={link.label} title={link.label} data-dock-app={link.id}
        onPointerEnter={() => setHover(link.to)}>{link.icon}</ShellLink>)}
      {windows.length > 0 ? <span className="dock-separator" aria-hidden="true" /> : null}
      {windows.length > 0 ? <div className="dock-windows" aria-label={t("desktop.running")}>
        {windows.map((item) => <button key={item.id} type="button" aria-label={item.title} title={item.title}
          aria-pressed={!item.minimized && active === item.id} className={`dock-item${magnify(item.id)}${item.minimized ? " is-minimized" : " is-running"}`}
          onPointerEnter={() => setHover(item.id)}
          onClick={() => { if (!item.minimized && active === item.id && mode === "desktop") { dispatch({ type: "toggle-minimize", id: item.id }); open("/"); }
            else { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); } }}><span aria-hidden="true">{(() => { const app = apps.find(app => item.id === `app:${app.id}` || item.id === app.path || item.location === app.path); return app ? <AppIcon app={app}/> : <GridIcon/>; })()}</span><i className={`dock-dot${item.minimized ? " is-minimized" : ""}`} aria-hidden="true" /></button>)}
      </div> : null}
      <span className="dock-separator" aria-hidden="true" />
      <button className={`dock-item dock-desktop${magnify("desktop")}`} type="button" onPointerEnter={() => setHover("desktop")}
        onClick={() => { dispatch({ type: "minimize-all" }); open("/"); }} aria-label={t("desktop.show")} title={t("desktop.show")}><span aria-hidden="true">▱</span></button>
    </nav>
    {launcher ? <StartMenu onClose={() => setLauncher(false)} /> : null}
  </>;
}
