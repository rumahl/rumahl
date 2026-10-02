import { RumahlSelect } from "../components/RumahlSelect";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { RumahlMark } from "../components/RumahlMark";
import { SearchIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { appPath } from "../routing/paths";
import { useShell } from "./ShellContext";
import { MenuDropdown } from "./MenuDropdown";

type MenuId = "system" | "go" | "view" | "window" | "user";

export function MenuBar() {
  const { snapshot, state, dispatch, open, mode, setMode } = useShell();
  const preferences = useShellPreferences();
  const { t, locale } = useI18n();
  const [openMenu, setOpenMenu] = useState<MenuId | null>(null);
  const [timeZone, setTimeZone] = useState("UTC");
  const bar = useRef<HTMLElement>(null);
  const anchors = useRef<{ [key: string]: HTMLElement | null }>({});
  useEffect(() => {
    document.documentElement.lang = locale;
    setTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
  }, [locale]);
  useEffect(() => {
    if (!openMenu) return;
    const onPointerDown = (event: PointerEvent) => { const target = event.target as Element | null; if (bar.current?.contains(event.target as Node)) return; if (target?.closest(".menubar__dropdown")) return; setOpenMenu(null); };
    const onKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape") setOpenMenu(null); };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [openMenu]);
  const formatter = useMemo(() => new Intl.DateTimeFormat(locale, {
    weekday: "short", hour: "2-digit", minute: "2-digit", timeZone
  }), [locale, timeZone]);
  const close = () => setOpenMenu(null);
  const act = (run: () => void) => () => { close(); run(); };
  const windows = [...state.windows].sort((left, right) => left.id.localeCompare(right.id));
  const protection = snapshot.systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");

  function menu(id: MenuId, label: ReactNode, entries: ReactNode) {
    const expanded = openMenu === id;
    return <div className="menubar__menu" key={id} ref={(element) => { anchors.current[id] = element; }}>
      <button className="menubar__item" type="button" aria-haspopup="menu" aria-expanded={expanded}
        onPointerEnter={() => { if (openMenu) setOpenMenu(id); }}
        onClick={() => setOpenMenu(expanded ? null : id)}>{label}</button>
      <MenuDropdown anchor={{ current: anchors.current[id] ?? null }} open={expanded} className="menubar__dropdown">{entries}</MenuDropdown>
    </div>;
  }

  return <header ref={bar} className="menubar topbar-glass">
    <nav className="menubar__menus menuitems" aria-label={t("menubar.label")}>
      {menu("system", <span className="menubar__brand brand" aria-label="rumahl OS"><RumahlMark /></span>, <>
        <p className="menubar__about">{t("menubar.about")}<small>{snapshot.shellBuildId.slice(0, 8)}</small></p>
        <div className="menubar__separator" role="separator" />
        <button role="menuitem" type="button" onClick={act(() => open("/settings"))}>{t("nav.settings")}</button>
        <button role="menuitem" type="button" onClick={act(() => open(appPath("app-manager")))}>{t("appManager.title")}</button>
        <div className="menubar__separator" role="separator" />
        <button role="menuitem" type="submit" form="rumahl-sign-out">{t("session.signOut")}</button>
      </>)}
      {menu("go", t("menubar.go"), <>
        <button role="menuitem" type="button" onClick={act(() => open("/"))}>{t("nav.home")}</button>
        <button role="menuitem" type="button" onClick={act(() => open("/apps"))}>{t("nav.apps")}</button>
        <button role="menuitem" type="button" onClick={act(() => open("/activity"))}>{t("nav.activity")}</button>
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
    <div className="menubar__status menuright">
      <button className="menubar__item menubar__spotlight" type="button" aria-label={t("search.system")} title={t("menubar.spotlight")}
        aria-expanded={state.commandPaletteOpen} onClick={() => dispatch({ type: "toggle-command-palette" })}><SearchIcon /></button>
      {preferences.error ? <span className="menubar__error" role="status">{t(`preferences.${preferences.error}`)}</span> : null}
      <div className="menubar__mode">
        <RumahlSelect label={t("mode.label")} disabled={!preferences.ready || preferences.saving}
          title={t("preferences.quickMode")} value={mode}
          onChange={value => setMode(value === "launcher" ? "launcher" : "desktop")}
          options={[{ value: "desktop", label: t("mode.desktop") }, { value: "launcher", label: t("mode.launcher") }]} />
      </div>
      <span className="menubar__protection" data-state={snapshot.systemStatus.protection} title={`${t("status.protection.label")}: ${protection}`}>
        <span className="menubar__dot" aria-hidden="true" />{protection}
      </span>
      <time className="menubar__clock" dateTime={new Date(snapshot.systemStatus.observedAtUnixMs).toISOString()}>
        {formatter.format(new Date(snapshot.systemStatus.observedAtUnixMs))}
      </time>
      <div className="menubar__menu" ref={(element) => { anchors.current.user = element; }}>
        <button className="menubar__item menubar__user" type="button" aria-haspopup="menu" aria-expanded={openMenu === "user"}
          aria-label={t("profile.open")} onClick={() => setOpenMenu(openMenu === "user" ? null : "user")}>
          {snapshot.user.displayName.slice(0, 1).toUpperCase()}
        </button>
        <MenuDropdown anchor={{ current: anchors.current.user ?? null }} open={openMenu === "user"} align="right" className="menubar__dropdown menubar__dropdown--right">
          <p className="menubar__about">{t("menubar.signedInAs", { name: snapshot.user.displayName })}</p>
          <div className="menubar__separator" role="separator" />
          <button role="menuitem" type="button" onClick={act(() => open("/settings"))}>{t("nav.settings")}</button>
          <button role="menuitem" type="submit" form="rumahl-sign-out">{t("session.signOut")}</button>
        </MenuDropdown>
      </div>
    </div>
    <form action="/logout" id="rumahl-sign-out" method="post" hidden />
  </header>;
}
