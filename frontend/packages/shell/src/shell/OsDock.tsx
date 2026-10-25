import { windowBelongsToLink } from "./dock-model";
import { AppIcon } from "../apps/AppTile";
import { isLaunching, useLaunching } from "../apps/launching";
import { RumahlMark } from "../components/RumahlMark";
import { useShellApps } from "../apps/useShellApps";
import { useEffect, useRef, useState } from "react";
import { useLocation } from "react-router";
import { HomeIcon, GridIcon, SettingsIcon, PulseIcon } from "../icons";
import { ShellLink } from "../routing/ShellLink";
import { describeRoute } from "../routing/routes";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";
import { StartMenu } from "./launcher/StartMenu";
import { useGlassHost } from "../glass-engine/useGlassEngine";
import { GLASS_MATERIALS, glassColorTint, glassMaterialWithTint } from "@rumahl/ui/glass";
import { useTheme } from "@rumahl/ui";
import { useThemeTuning } from "../preferences/theme-tuning";

export function OsDock({ autoHide = false }: { autoHide?: boolean | undefined }) {
  const dockRef = useRef<HTMLElement>(null);
  const { tokens } = useTheme();
  const { tuning } = useThemeTuning();
  const tint = glassColorTint(tuning.glassColor, tuning.glassTint, tokens["color.accent"] ?? "#beff2d");
  useGlassHost(dockRef, glassMaterialWithTint(GLASS_MATERIALS.dock, tint));
  const { t } = useI18n();
  const apps = useShellApps();
  const { state, dispatch, open, mode } = useShell();
  useLaunching();
  const location = useLocation();
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const [launcher, setLauncher] = useState(false);
  const [hover, setHover] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [hoverDock, setHoverDock] = useState(false);
  useEffect(() => {
    if (!autoHide || !revealed || hoverDock) return;
    const timer = window.setTimeout(() => setRevealed(false), 1800);
    return () => window.clearTimeout(timer);
  }, [autoHide, revealed, hoverDock]);
  useEffect(() => {
    if (!autoHide) { setRevealed(false); return; }
    const reveal = () => setRevealed(true);
    const hide = () => setRevealed(false);
    const onMove = (event: PointerEvent) => {
      if (event.pointerType === "touch") return;
      if (event.clientY >= window.innerHeight - 24) reveal();
      else if (event.clientY < window.innerHeight - 96) hide();
    };
    let startY: number | null = null;
    const onTouchStart = (event: TouchEvent) => {
      startY = event.touches[0]?.clientY ?? null;
      if (startY !== null && startY >= window.innerHeight - 28) reveal();
      else hide();
    };
    const onTouchMove = (event: TouchEvent) => {
      const y = event.touches[0]?.clientY ?? 0;
      if (startY !== null && startY >= window.innerHeight - 28 && startY - y > 18) reveal();
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    window.addEventListener("touchstart", onTouchStart, { passive: true });
    window.addEventListener("touchmove", onTouchMove, { passive: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchmove", onTouchMove);
    };
  }, [autoHide]);
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
    <nav ref={dockRef} className={`dock os-dock rumahl-glass-host os-dock--${mode}${autoHide ? " is-auto-hide" : ""}${autoHide && revealed ? " is-revealed" : ""}`} aria-label={t("nav.main")} onPointerEnter={() => { setHoverDock(true); if (autoHide) setRevealed(true); }} onFocusCapture={() => { setHoverDock(true); if (autoHide) setRevealed(true); }} onBlurCapture={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setHoverDock(false); }} onPointerLeave={() => { setHoverDock(false); setHover(null); }}>
      {mode === "desktop" ? <button className={`dock-item dock-icon dock-start${magnify("start")}`} type="button" aria-label={t("launcher.open")} aria-expanded={launcher}
        onPointerEnter={() => setHover("start")}
        onClick={() => setLauncher((value) => !value)}><RumahlMark /></button> : null}
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
