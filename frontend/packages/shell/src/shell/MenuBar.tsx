import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useLocation } from "react-router";
import { RumahlMark } from "../components/RumahlMark";
import { DesktopIcon, GridIcon, SearchIcon, SlidersIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useShell } from "./ShellContext";
import { useDeviceViewport } from "./device";
import { useClock } from "./clock";
import { ControlCenter } from "./ControlCenter";
import { openMobileTour } from "./MobileTutorial";
import { MenuDropdown } from "./MenuDropdown";
import { featureReady, useFeatures } from "../features/ShellFeatures";
import { flushSessionSnapshot } from "./session-snapshot";

type MenuId = "system" | "go" | "view" | "window" | "user";

export function MenuBar() {
  const { snapshot, state, dispatch, open, mode, setMode } = useShell();
  const location = useLocation();
  const viewport = useDeviceViewport();
  const features = useFeatures();
  const preferences = useShellPreferences();
  const { t, locale } = useI18n();
  const now = useClock(1000);
  const [openMenu, setOpenMenu] = useState<MenuId | null>(null);
  const [controlOpen, setControlOpen] = useState(false);
  const [topRevealed, setTopRevealed] = useState(false);
  const hideTop = useRef<number | undefined>(undefined);
  const [timeZone, setTimeZone] = useState("UTC");
  const bar = useRef<HTMLElement>(null);
  const anchors = useRef<{ [key: string]: HTMLElement | null }>({});
  const signingOut = useRef(false);
  useEffect(() => {
    document.documentElement.lang = locale;
    setTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
  }, [locale]);
  const atHome = location.pathname === "/";
  // "An app is fullscreen": a launcher app is open, or a window is maximized.
  // The island tucks upward so the window can use the space; a slim sliver
  // remains at the top edge and reveals the full bar on approach.
  const appFullscreen = mode === "launcher" || mode === "mobile"
    ? !atHome && state.windows.some((item) => !item.minimized)
    : state.windows.some((item) => !item.minimized && (item.fullscreen || item.placement === "maximized"));
  useEffect(() => {
    if (!appFullscreen) { setTopRevealed(false); return; }
    const reveal = () => { if (hideTop.current) window.clearTimeout(hideTop.current); setTopRevealed(true); };
    const scheduleHide = () => {
      if (hideTop.current) window.clearTimeout(hideTop.current);
      hideTop.current = window.setTimeout(() => setTopRevealed(false), 550);
    };
    const onMove = (event: PointerEvent) => {
      if (event.pointerType === "touch") return;
      if (event.clientY <= 26) reveal();
      else if (event.clientY > 70) scheduleHide();
    };
    let startY: number | null = null;
    const onTouchStart = (event: TouchEvent) => {
      startY = event.touches[0]?.clientY ?? null;
      if (startY !== null && startY <= 26) reveal();
      else setTopRevealed(false);
    };
    const onTouchMove = (event: TouchEvent) => {
      const y = event.touches[0]?.clientY ?? 0;
      if (startY !== null && startY <= 26 && y - startY > 18) reveal();
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    window.addEventListener("touchstart", onTouchStart, { passive: true });
    window.addEventListener("touchmove", onTouchMove, { passive: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchmove", onTouchMove);
      if (hideTop.current) window.clearTimeout(hideTop.current);
    };
  }, [appFullscreen]);
  useEffect(() => {
    if (!openMenu && !controlOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (bar.current?.contains(event.target as Node)) return;
      if (target?.closest(".menubar__dropdown")) return;
      if (target?.closest(".control-center")) return;
      setOpenMenu(null);
      setControlOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape") { setOpenMenu(null); setControlOpen(false); } };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [openMenu, controlOpen]);
  const formatter = useMemo(() => new Intl.DateTimeFormat(locale, {
    weekday: "short", hour: "2-digit", minute: "2-digit", timeZone
  }), [locale, timeZone]);
  const close = () => setOpenMenu(null);
  const act = (run: () => void) => () => { close(); run(); };
  const windows = [...state.windows].sort((left, right) => left.id.localeCompare(right.id));
  const protection = snapshot.systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  // Live clock after mount; falls back to the snapshot time for the first paint.
  const time = now ?? new Date(snapshot.systemStatus.observedAtUnixMs);

  const topRevealedNow = topRevealed || openMenu !== null || controlOpen;
  const autoHide = appFullscreen;

  function menu(id: MenuId, label: ReactNode, entries: ReactNode) {
    const expanded = openMenu === id;
    return <div className="menubar__menu" key={id} ref={(element) => { anchors.current[id] = element; }}>
      <button className="menubar__item" type="button" aria-haspopup="menu" aria-expanded={expanded}
        onPointerEnter={() => { if (openMenu) setOpenMenu(id); }}
        onClick={() => setOpenMenu(expanded ? null : id)}>{label}</button>
      <MenuDropdown anchor={{ current: anchors.current[id] ?? null }} open={expanded} className="menubar__dropdown">{entries}</MenuDropdown>
    </div>;
  }

  return <header ref={bar} className={`menubar topbar${autoHide ? " is-auto-hide" : ""}${autoHide && topRevealedNow ? " is-revealed" : ""}`}>
    <div className="topbar__island topbar__island--left">
      <nav className="menubar__menus" aria-label={t("menubar.label")}>
        {menu("system", <span className="menubar__brand brand" aria-label="rumahl OS"><RumahlMark /><span className="menubar__wordmark" aria-hidden="true">rumahl</span></span>, <>
          <p className="menubar__about">{t("menubar.about")}<small>{snapshot.shellBuildId.slice(0, 8)}</small></p>
          <div className="menubar__separator" role="separator" />
          <button role="menuitem" type="button" onClick={act(() => open("/settings"))}>{t("nav.settings")}</button>
          <div className="menubar__separator" role="separator" />
          <button role="menuitem" type="submit" form="rumahl-sign-out" disabled={!featureReady(features["session.signOut"])} title={!featureReady(features["session.signOut"]) ? t("features.demoUnavailable") : undefined}>{t("session.signOut")}</button>
        </>)}
        {menu("go", t("menubar.go"), <>
          <button role="menuitem" type="button" onClick={act(() => open("/"))}>{t("nav.home")}</button>
          <button role="menuitem" type="button" disabled={!featureReady(features.activity)} title={!featureReady(features.activity) ? t("features.demoUnavailable") : undefined} onClick={act(() => open("/activity"))}>{t("nav.activity")}</button>
          <button role="menuitem" type="button" onClick={act(() => open("/settings"))}>{t("nav.settings")}</button>
        </>)}
        {menu("view", t("menubar.view"), <>
          <button role="menuitemradio" aria-checked={mode === "desktop"} type="button" onClick={act(() => setMode("desktop"))}>{t("mode.desktop")}</button>
          <button role="menuitemradio" aria-checked={mode === "launcher"} type="button" onClick={act(() => setMode("launcher"))}>{t("mode.launcher")}</button>
          <div className="menubar__separator" role="separator" />
          <button role="menuitem" type="button" onClick={act(() => dispatch({ type: "toggle-command-palette" }))}>{t("menubar.spotlight")}</button>
          <button role="menuitem" type="button" onClick={act(() => { dispatch({ type: "minimize-all" }); open("/"); })}>{t("desktop.menu.show")}</button>
        </>)}
        {menu("window", t("menubar.window"), <>
          <button role="menuitem" type="button" disabled={windows.length === 0} onClick={act(() => dispatch({ type: "minimize-all" }))}>{t("menubar.minimizeAll")}</button>
          <div className="menubar__separator" role="separator" />
          {windows.length === 0
            ? <p className="menubar__empty">{t("menubar.noWindows")}</p>
            : windows.map((item) => <button role="menuitem" type="button" key={item.id}
                onClick={act(() => { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); })}>{item.title}</button>)}
        </>)}
      </nav>
    </div>

    <div className="topbar__island topbar__island--right">
      <button className="topbar__button menubar__spotlight" type="button" aria-label={t("search.system")} title={t("menubar.spotlight")}
        aria-expanded={state.commandPaletteOpen} onClick={() => dispatch({ type: "toggle-command-palette" })}><SearchIcon /></button>
      <div className="topbar__anchor" ref={(element) => { anchors.current.control = element; }}>
        <button className="topbar__button" type="button" aria-haspopup="dialog" aria-expanded={controlOpen}
          aria-label={t("controlCenter.label")} title={t("controlCenter.label")} onClick={() => setControlOpen((value) => !value)}><SlidersIcon /></button>
        <MenuDropdown anchor={{ current: anchors.current.control ?? null }} open={controlOpen} align="right" className="menubar__dropdown menubar__dropdown--right menubar__dropdown--control">
          <ControlCenter protection={protection} state={snapshot.systemStatus.protection} error={preferences.error} userName={snapshot.user.displayName} windowCount={state.windows.length} onOpenSettings={act(() => open("/settings"))} onOpenSwitcher={() => { setControlOpen(false); dispatch({ type: "toggle-app-switcher" }); }} onOpenTour={(mode === "launcher" || mode === "mobile") && viewport.phone ? () => { setControlOpen(false); openMobileTour(); } : undefined} onClose={() => setControlOpen(false)} />
        </MenuDropdown>
      </div>
      <div className="topbar__mode" role="group" aria-label={t("mode.label")}>
        <button className="topbar__button" type="button" aria-pressed={mode === "desktop"} aria-label={t("mode.desktop")} title={t("mode.desktop")}
          disabled={!preferences.ready || preferences.saving} onClick={() => setMode("desktop")}><DesktopIcon /></button>
        <button className="topbar__button" type="button" aria-pressed={mode === "launcher"} aria-label={t("mode.launcher")} title={t("mode.launcher")}
          disabled={!preferences.ready || preferences.saving} onClick={() => setMode("launcher")}><GridIcon /></button>
      </div>
      <time className="menubar__clock" dateTime={time.toISOString()}>{formatter.format(time)}</time>
      <div className="menubar__menu" ref={(element) => { anchors.current.user = element; }}>
        <button className="menubar__item menubar__user" type="button" aria-haspopup="menu" aria-expanded={openMenu === "user"}
          aria-label={t("profile.open")} onClick={() => setOpenMenu(openMenu === "user" ? null : "user")}>
          {snapshot.user.displayName.slice(0, 1).toUpperCase()}
        </button>
        <MenuDropdown anchor={{ current: anchors.current.user ?? null }} open={openMenu === "user"} align="right" className="menubar__dropdown menubar__dropdown--right">
          <p className="menubar__about">{t("menubar.signedInAs", { name: snapshot.user.displayName })}</p>
          <div className="menubar__separator" role="separator" />
          <button role="menuitem" type="button" onClick={act(() => open("/settings"))}>{t("nav.settings")}</button>
          <button role="menuitem" type="submit" form="rumahl-sign-out" disabled={!featureReady(features["session.signOut"])} title={!featureReady(features["session.signOut"]) ? t("features.demoUnavailable") : undefined}>{t("session.signOut")}</button>
        </MenuDropdown>
      </div>
    </div>
    <form action="/logout" id="rumahl-sign-out" method="post" hidden onSubmit={(event) => {
      if (signingOut.current) return;
      event.preventDefault();
      signingOut.current = true;
      const form = event.currentTarget;
      const timeout = new Promise((resolve) => window.setTimeout(resolve, 600));
      void Promise.race([flushSessionSnapshot(), timeout]).finally(() => form.submit());
    }} />
  </header>;
}
