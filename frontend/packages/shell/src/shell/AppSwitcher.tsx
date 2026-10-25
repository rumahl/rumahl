import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { HomeIcon, WindowCloseIcon, WindowStackIcon } from "../icons";
import { AppIcon } from "../apps/AppTile";
import { useShellApps } from "../apps/useShellApps";
import { useShell } from "./ShellContext";
import { WindowPreview } from "./WindowPreview";

/** The native dialog keeps keyboard focus inside the overview and makes the
 * running apps inert without unmounting or restarting their frames. */
export function AppSwitcher() {
  const { state, dispatch, open } = useShell();
  const { t } = useI18n();
  const apps = useShellApps();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const element = dialog.current;
    if (!state.appSwitcherOpen || !element) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    element.showModal();
    element.querySelector(".is-current")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "instant" });
    return () => {
      element.close();
      if (previous?.isConnected && !previous.closest(".is-hidden")) previous.focus();
    };
  }, [state.appSwitcherOpen]);

  if (!state.appSwitcherOpen || typeof document === "undefined") return null;
  const close = () => dispatch({ type: "close-app-switcher" });
  const windows = [...state.windows];
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
      <section className="app-switcher">
        <header className="app-switcher__head">
          <h2>{t("switcher.title")}</h2>
          <button type="button" className="app-switcher__close" aria-label={t("files.dismiss")} onClick={close}><WindowCloseIcon /></button>
        </header>
        {windows.length === 0 ? <p className="app-switcher__empty">{t("switcher.empty")}</p> : <div className="app-switcher__grid">
          {windows.map((item) => {
            const app = apps.find(app => item.id === `app:${app.id}` || item.location === app.path || (app.path === "/settings" && item.location?.startsWith("/settings/")));
            const icon = app ? <AppIcon app={app} /> : <WindowStackIcon />;
            return <article key={item.id} className={`app-switcher__card${state.focusedWindowId === item.id ? " is-current" : ""}`}>
              <WindowPreview windowId={item.id} fallback={<><span className="app-switcher__icon">{icon}</span><strong>{item.title}</strong></>} />
              <button type="button" className="app-switcher__open" onClick={() => {
                close();
                if (item.location) open(item.location);
                dispatch({ type: "focus-window", id: item.id });
              }}>
                <span className="app-switcher__icon" aria-hidden="true">{icon}</span>
                <span className="app-switcher__title">{item.title}</span>
              </button>
              <button type="button" className="app-switcher__remove" aria-label={t("window.close", { title: item.title })} onClick={() => {
                dispatch({ type: "close-window", id: item.id });
                if (state.focusedWindowId === item.id) open("/");
              }}><WindowCloseIcon /></button>
            </article>;
          })}
        </div>}
        <footer className="app-switcher__footer">
          <button type="button" className="app-switcher__home" onClick={() => { close(); open("/"); }}><HomeIcon aria-hidden="true" />{t("nav.home")}</button>
        </footer>
      </section>
    </dialog>,
    document.querySelector(".shell") ?? document.body
  );
}
