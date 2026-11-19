import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { HomeIcon, WindowCloseIcon, WindowStackIcon } from "../icons";
import { AppIcon } from "../apps/AppTile";
import { useShellApps } from "../apps/useShellApps";
import { useShell } from "./ShellContext";
import { WindowPreview } from "./WindowPreview";
import type { ShellWindow } from "../shell-state";

/** The native dialog keeps keyboard focus inside the overview and makes the
 * running apps inert without unmounting or restarting their frames. */
export function AppSwitcher() {
  const { state, dispatch, open, mode } = useShell();
  const { t } = useI18n();
  const apps = useShellApps();
  const dialog = useRef<HTMLDialogElement>(null);
  const grid = useRef<HTMLDivElement>(null);
  const rail = useRef<HTMLDivElement>(null);
  const opened = useRef(false);
  const anchored = useRef(false);
  const pendingCenter = useRef<string | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const mobile = mode === "mobile";

  const centerCard = useCallback((index: number, behavior: ScrollBehavior = "smooth") => {
    const scroller = grid.current;
    const card = scroller?.querySelectorAll<HTMLElement>(".app-switcher__card")[index];
    if (!scroller || !card) return;
    const scrollerBounds = scroller.getBoundingClientRect();
    const cardBounds = card.getBoundingClientRect();
    const left = scroller.scrollLeft + cardBounds.left - scrollerBounds.left - (scrollerBounds.width - cardBounds.width) / 2;
    if (typeof scroller.scrollTo === "function") scroller.scrollTo({ left, behavior });
    else scroller.scrollLeft = left;
  }, []);

  const focusCard = useCallback((id: string) => {
    const cards = [...(grid.current?.querySelectorAll<HTMLElement>("[data-window-id]") ?? [])];
    cards.find((card) => card.dataset.windowId === id)?.querySelector<HTMLButtonElement>(".app-switcher__open")?.focus({ preventScroll: true });
  }, []);

  const scrollRail = useCallback((id: string) => {
    const container = rail.current;
    const item = container?.querySelector<HTMLElement>(`[data-rail-id="${id}"]`);
    if (!container || !item) return;
    const left = item.offsetLeft - (container.clientWidth - item.offsetWidth) / 2;
    if (typeof container.scrollTo === "function") container.scrollTo({ left, behavior: "smooth" });
    else container.scrollLeft = left;
  }, []);

  useEffect(() => {
    const element = dialog.current;
    if (!state.appSwitcherOpen || !element) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    element.showModal();
    return () => {
      element.close();
      if (previous?.isConnected && !previous.closest(".is-hidden")) previous.focus();
    };
  }, [state.appSwitcherOpen]);

  // The focused app anchors the deck whenever the overview opens.
  useLayoutEffect(() => {
    if (!state.appSwitcherOpen) { opened.current = false; return; }
    if (opened.current) return;
    opened.current = true;
    setActiveId(state.focusedWindowId ?? state.windows[0]?.id ?? null);
  }, [state.appSwitcherOpen, state.focusedWindowId, state.windows.length, state.windows]);

  // Center the focused card and take focus once per opening. The desktop
  // dialog is only measurable after showModal, hence the frame.
  useEffect(() => {
    if (!state.appSwitcherOpen) { anchored.current = false; return; }
    if (anchored.current) return;
    anchored.current = true;
    const id = state.focusedWindowId ?? state.windows[0]?.id ?? null;
    const index = state.windows.findIndex((item) => item.id === id);
    const frame = requestAnimationFrame(() => {
      if (index >= 0) centerCard(index, "auto");
      if (id) focusCard(id);
    });
    return () => cancelAnimationFrame(frame);
  }, [state.appSwitcherOpen, state.focusedWindowId, state.windows.length, state.windows, centerCard, focusCard]);

  // Closing the active card changes the deck length; focus the next card.
  useEffect(() => {
    const id = pendingCenter.current;
    if (!state.appSwitcherOpen || !mobile || !id) return;
    const index = state.windows.findIndex((item) => item.id === id);
    if (index < 0) return;
    pendingCenter.current = null;
    const frame = requestAnimationFrame(() => { centerCard(index, "auto"); scrollRail(id); });
    return () => cancelAnimationFrame(frame);
  }, [state.windows, state.appSwitcherOpen, mobile, centerCard, scrollRail]);

  useEffect(() => {
    if (!state.appSwitcherOpen) return;
    if (activeId && state.windows.some((item) => item.id === activeId)) return;
    setActiveId(state.windows[0]?.id ?? null);
  }, [state.appSwitcherOpen, activeId, state.windows]);

  useEffect(() => {
    if (!state.appSwitcherOpen || state.windows.length > 0) return;
    dispatch({ type: "close-app-switcher" });
    open("/");
  }, [state.appSwitcherOpen, state.windows.length, dispatch, open]);

  useEffect(() => {
    if (!mobile || !state.appSwitcherOpen || !activeId) return;
    scrollRail(activeId);
  }, [mobile, state.appSwitcherOpen, activeId, scrollRail]);

  useEffect(() => {
    if (!mobile || !state.appSwitcherOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") dispatch({ type: "close-app-switcher" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mobile, state.appSwitcherOpen, dispatch]);

  if (!state.appSwitcherOpen || typeof document === "undefined") return null;
  const close = () => dispatch({ type: "close-app-switcher" });
  const windows = [...state.windows];
  const mobileActiveIndex = Math.max(0, windows.findIndex((item) => item.id === activeId));

  const closeWindow = (id: string) => {
    const remaining = windows.filter((item) => item.id !== id);
    dispatch({ type: "close-window", id });
    if (remaining.length === 0) return; // the redirect effect takes over
    if (activeId === id) {
      const next = remaining[0]!.id;
      setActiveId(next);
      pendingCenter.current = next;
    }
    if (state.focusedWindowId === id) open("/");
  };

  const openWindow = (item: ShellWindow) => {
    close();
    if (item.location) open(item.location);
    dispatch({ type: "focus-window", id: item.id });
  };

  const selectApp = (id: string) => {
    const index = windows.findIndex((item) => item.id === id);
    if (index < 0) return;
    setActiveId(id);
    centerCard(index, "auto");
    requestAnimationFrame(() => focusCard(id));
  };

  const syncActive = () => {
    const scroller = grid.current;
    if (!scroller) return;
    const bounds = scroller.getBoundingClientRect();
    const middle = bounds.left + bounds.width / 2;
    let nearest = 0;
    let distance = Number.POSITIVE_INFINITY;
    scroller.querySelectorAll<HTMLElement>(".app-switcher__card").forEach((card, index) => {
      const rect = card.getBoundingClientRect();
      const delta = Math.abs(rect.left + rect.width / 2 - middle);
      if (delta < distance) { distance = delta; nearest = index; }
    });
    const id = windows[nearest]?.id;
    if (id) setActiveId((current) => current === id ? current : id);
  };

  const iconFor = (item: ShellWindow) => {
    const app = apps.find(app => item.id === `app:${app.id}` || item.location === app.path || (app.path === "/settings" && item.location?.startsWith("/settings/")));
    const icon = app ? <AppIcon app={app} /> : <WindowStackIcon />;
    return <span className="app-switcher__icon" aria-hidden="true">{icon}</span>;
  };

  const card = (item: ShellWindow, current: boolean) => {
    const index = windows.findIndex((entry) => entry.id === item.id);
    const distance = index - mobileActiveIndex;
    const level = Math.min(2, Math.abs(distance));
    const style = mobile ? {
      transform: `translateY(${level * 7}px) scale(${1 - level * 0.045})`,
      opacity: Math.max(0.55, 1 - Math.abs(distance) * 0.18),
      zIndex: Math.max(1, 30 - Math.abs(distance) * 10)
    } : undefined;
    return <article key={item.id} data-window-id={item.id}
      className={`app-switcher__card${current ? " is-current" : ""}`}
      aria-current={current ? "true" : undefined}
      style={style}>
      <WindowPreview windowId={item.id} fallback={<>{iconFor(item)}<strong>{item.title}</strong></>} />
      {mobile
        ? <button type="button" className="app-switcher__open app-switcher__open--cover"
            aria-label={current ? t("switcher.openApp", { title: item.title }) : t("switcher.bringForward", { title: item.title })}
            onClick={() => { if (item.id === activeId) openWindow(item); else selectApp(item.id); }} />
        : <button type="button" className="app-switcher__open" aria-label={t("switcher.openApp", { title: item.title })} onClick={() => openWindow(item)}>
            {iconFor(item)}
            <span className="app-switcher__title">{item.title}</span>
          </button>}
      {!mobile || current
        ? <button type="button" className="app-switcher__remove" aria-label={t("window.close", { title: item.title })} onClick={(event) => {
            event.stopPropagation();
            closeWindow(item.id);
          }}><WindowCloseIcon /></button>
        : null}
    </article>;
  };

  const content = <section className={`app-switcher${mobile ? " app-switcher--mobile" : ""}`}>
    {mobile ? null : <header className="app-switcher__head">
      <h2>{t("switcher.title")}</h2>
      <button type="button" className="app-switcher__close" aria-label={t("files.dismiss")} onClick={close}><WindowCloseIcon /></button>
    </header>}
    {windows.length === 0
      ? <p className="app-switcher__empty">{t("switcher.empty")}</p>
      : mobile
        ? <>
            <div ref={rail} className="app-switcher__rail" role="group" aria-label={t("switcher.title")}>
              {windows.map((item) => <button key={item.id} type="button" data-rail-id={item.id}
                className={`app-switcher__rail-item${item.id === activeId ? " is-current" : ""}`}
                aria-pressed={item.id === activeId}
                aria-label={item.id === activeId ? item.title : t("switcher.bringForward", { title: item.title })}
                onClick={() => selectApp(item.id)}>
                {iconFor(item)}
                {item.id === activeId ? <span className="app-switcher__rail-title">{item.title}</span> : null}
              </button>)}
            </div>
            <div ref={grid} className="app-switcher__grid" onScroll={syncActive}>
              {windows.map((item) => card(item, item.id === activeId))}
            </div>
          </>
        : <div ref={grid} className="app-switcher__grid">
            {windows.map((item) => card(item, item.id === activeId))}
          </div>}
    <footer className="app-switcher__footer">
      <button type="button" className="app-switcher__home" onClick={() => { close(); open("/"); }}><HomeIcon aria-hidden="true" />{t("nav.home")}</button>
    </footer>
  </section>;

  if (mobile) {
    return createPortal(
      <div className="app-switcher-overlay app-switcher-overlay--mobile" role="dialog" aria-modal="true" aria-label={t("switcher.title")}>
        {content}
      </div>,
      document.querySelector(".shell") ?? document.body
    );
  }

  return createPortal(
    <dialog ref={dialog} className="app-switcher-overlay" aria-label={t("switcher.title")} onCancel={(event) => { event.preventDefault(); close(); }}
      onClick={(event) => { if (event.target === event.currentTarget) close(); }}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        const controls = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")].filter(button => !button.closest("[inert]"));
        const first = controls[0], last = controls.at(-1);
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }}>
      {content}
    </dialog>,
    document.querySelector(".shell") ?? document.body
  );
}
