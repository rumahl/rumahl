import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { AppIcon, AppTile } from "../../apps/AppTile";
import { resolveRecentApp, useRecents } from "../../apps/recents";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useWorkspace } from "../../preferences/Workspace";
import { useShell } from "../ShellContext";
import { useClock } from "../clock";

const PAGE_SIZE = 24;

/**
 * The launcher: one coherent app surface. A greeting, clock and search up top;
 * recently used apps, workspace folders and the full app grid below. Folders
 * open in place.
 */
export function LauncherHome() {
  const { t, locale } = useI18n();
  const { snapshot } = useShell();
  const apps = useShellApps();
  const { effective } = useWorkspace();
  const recents = useRecents();
  const [query, setQuery] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const now = useClock();
  const swipe = useRef<number | null>(null);
  useEffect(() => { setPage(0); }, [query, folder]);

  const normalized = query.trim().toLocaleLowerCase();
  const currentFolder = effective.folders.find((item) => item.id === folder);
  const visible = apps.filter((app) => {
    if (normalized) return `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized);
    if (currentFolder) return currentFolder.apps.includes(app.id);
    return !effective.folders.some((item) => item.apps.includes(app.id));
  });
  const pages = useMemo(() => {
    const result: ShellApp[][] = [];
    for (let index = 0; index < visible.length; index += PAGE_SIZE) result.push(visible.slice(index, index + PAGE_SIZE));
    return result.length ? result : [[]];
  }, [visible]);
  const activePage = Math.min(page, pages.length - 1);
  const recentApps = useMemo(() => {
    const seen = new Set<string>();
    const result: ShellApp[] = [];
    for (const path of recents) {
      const app = resolveRecentApp(path, apps);
      if (app && !seen.has(app.id)) { seen.add(app.id); result.push(app); }
    }
    return result.slice(0, 6);
  }, [recents, apps]);

  const clock = now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "";
  const date = now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : "";
  const hour = now ? now.getHours() : null;
  const greeting = hour === null ? "launcher.greetingHello"
    : hour < 5 ? "launcher.greetingNight"
    : hour < 12 ? "launcher.greetingMorning"
    : hour < 18 ? "launcher.greetingAfternoon"
    : "launcher.greetingEvening";
  const step = (delta: number) => setPage((value) => Math.min(Math.max(value + delta, 0), pages.length - 1));

  function onPointerDown(event: ReactPointerEvent) {
    if (event.pointerType === "mouse") return;
    swipe.current = event.clientX;
  }
  function onPointerUp(event: ReactPointerEvent) {
    const start = swipe.current;
    swipe.current = null;
    if (start === null) return;
    const distance = event.clientX - start;
    if (Math.abs(distance) >= 60) step(distance < 0 ? 1 : -1);
  }
  function onKeyDown(event: ReactKeyboardEvent) {
    if ((event.target as HTMLElement).closest("input, textarea, select")) return;
    if (event.key === "ArrowRight") { event.preventDefault(); step(1); }
    else if (event.key === "ArrowLeft") { event.preventDefault(); step(-1); }
  }

  return <main className="launcher-home">
    <header className="launcher-header">
      <div className="launcher-heading">
        <h1 className="launcher-greeting">{t(greeting)}<span>{snapshot.user.displayName}</span></h1>
        {now ? <p className="launcher-clock"><strong>{clock}</strong><span>{date}</span></p> : null}
      </div>
      <label className="app-search launcher-search">
        <span>{t("launcher.search")}</span>
        <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("launcher.searchPlaceholder")} />
      </label>
    </header>

    <div className="launcher-view launcher-view--grid" tabIndex={0} aria-label={t("launcher.pages")}
      onPointerDown={onPointerDown} onPointerUp={onPointerUp} onPointerCancel={() => { swipe.current = null; }} onKeyDown={onKeyDown}>
      {!normalized && !currentFolder && recentApps.length > 0 ? <section className="launcher-section launcher-section--recent">
        <h2>{t("launcher.recent")}</h2>
        <div className="app-grid">{recentApps.map((app) => <AppTile app={app} key={app.id} />)}</div>
      </section> : null}
      <section className="launcher-section">
        <h2>{currentFolder ? currentFolder.name : t("launcher.allApps")}</h2>
        {currentFolder ? <button className="launcher-back" type="button" onClick={() => setFolder(null)}>{t("workspace.back")} · {currentFolder.name}</button> : null}
        <div className="app-grid">
          {!normalized && !currentFolder ? effective.folders.map((item) => <button className="app-tile" key={item.id} type="button" onClick={() => setFolder(item.id)}>
            <span className="app-tile__icon folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = apps.find((a) => a.id === id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span><span>{item.name}</span>
          </button>) : null}
          {pages[activePage]!.map((app) => <AppTile app={app} key={app.id} />)}
        </div>
        {visible.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
      </section>
      {pages.length > 1 ? <nav className="launcher-pages" aria-label={t("launcher.pages")}>
        {pages.map((_, index) => <button key={index} type="button" aria-current={index === activePage ? "page" : undefined}
          aria-label={t("launcher.page", { page: index + 1 })} onClick={() => setPage(index)} />)}
      </nav> : null}
    </div>
  </main>;
}
