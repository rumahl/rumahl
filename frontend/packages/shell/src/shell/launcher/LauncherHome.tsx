import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { AppIcon, AppTile } from "../../apps/AppTile";
import { resolveRecentApp, useRecents } from "../../apps/recents";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useWorkspace } from "../../preferences/Workspace";
import { ArrowIcon } from "../../icons";
import { useTheme } from "@rumahl/ui";
import { useShell } from "../ShellContext";
import { useClock } from "../clock";
import { useLauncherView, type LauncherView } from "./launcher-view";

const PAGE_SIZE = 24;
const VIEWS: readonly LauncherView[] = ["grid", "deck", "canvas"];

export function LauncherHome() {
  const { t, locale } = useI18n();
  const { snapshot, open } = useShell();
  const apps = useShellApps();
  const { effective } = useWorkspace();
  const { variants } = useTheme();
  const recents = useRecents();
  const [view, setView] = useLauncherView(variants.launcherLayout === "drawer" ? "deck" : "grid");
  const [query, setQuery] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const now = useClock();
  const swipe = useRef<number | null>(null);
  useEffect(() => { setPage(0); }, [query, view, folder]);

  const normalized = query.trim().toLocaleLowerCase();
  const currentFolder = effective.folders.find((item) => item.id === folder);
  const visible = apps.filter((app) => {
    if (normalized) return `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized);
    if (currentFolder) return currentFolder.apps.includes(app.id);
    return view !== "grid" || !effective.folders.some(item => item.apps.includes(app.id));
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
  const groups = useMemo(() => {
    const map = new Map<string, ShellApp[]>();
    for (const app of visible) {
      const letter = /^[a-z]/i.test(app.title) ? app.title[0]!.toUpperCase() : "#";
      const list = map.get(letter) ?? [];
      list.push(app);
      map.set(letter, list);
    }
    return [...map.entries()].sort(([left], [right]) => left === "#" ? 1 : right === "#" ? -1 : left.localeCompare(right));
  }, [visible]);

  const clock = now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "";
  const date = now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : "";
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

  const search = <label className="app-search launcher-search">
    <span>{t("launcher.search")}</span>
    <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("launcher.searchPlaceholder")} />
  </label>;

  return <main className="launcher-home">
    <header className="launcher-header">
      <div className="launcher-heading">
        <span className="launcher-wordmark">rumahl</span>
        <p>{snapshot.user.displayName}</p>
        <h1>{t("launcher.heading")}</h1>
        {now ? <p className="launcher-clock"><strong>{clock}</strong><span>{date}</span></p> : null}
      </div>
      <div className="launcher-views" role="group" aria-label={t("launcher.view.label")}>
        {VIEWS.map((candidate) => <button key={candidate} type="button" aria-pressed={view === candidate}
          onClick={() => { setView(candidate); setFolder(null); }}>{t(`launcher.view.${candidate}`)}</button>)}
      </div>
    </header>

    {view === "grid" ? <div className="launcher-view launcher-view--grid" tabIndex={0} aria-label={t("launcher.pages")}
      onPointerDown={onPointerDown} onPointerUp={onPointerUp} onPointerCancel={() => { swipe.current = null; }} onKeyDown={onKeyDown}>
      {search}
      {!normalized && !currentFolder && recentApps.length > 0 ? <section className="launcher-section launcher-section--recent">
        <h2>{t("launcher.recent")}</h2>
        <div className="app-grid">{recentApps.map((app) => <AppTile app={app} key={app.id} />)}</div>
      </section> : null}
      <section className="launcher-section">
        <h2>{currentFolder ? currentFolder.name : t("launcher.allApps")}</h2>
        {currentFolder ? <button className="launcher-back" type="button" onClick={() => setFolder(null)}>{t("workspace.back")}</button> : null}
        <div className="app-grid">
          {!normalized && !currentFolder ? effective.folders.map((item) => <button className="app-tile" key={item.id} type="button" onClick={() => setFolder(item.id)}>
            <span className="app-tile__icon folder-preview" aria-hidden="true">{item.apps.slice(0,4).map(id => { const app = apps.find(a => a.id === id); return app ? <span key={id}><AppIcon app={app}/></span> : null; })}</span><span>{item.name}</span>
          </button>) : null}
          {pages[activePage]!.map((app) => <AppTile app={app} key={app.id} />)}
        </div>
        {visible.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
      </section>
      {pages.length > 1 ? <nav className="launcher-pages" aria-label={t("launcher.pages")}>
        {pages.map((_, index) => <button key={index} type="button" aria-current={index === activePage ? "page" : undefined}
          aria-label={t("launcher.page", { page: index + 1 })} onClick={() => setPage(index)} />)}
      </nav> : null}
    </div> : null}

    {view === "deck" ? <div className="launcher-view launcher-view--drawer">
      <div className="drawer-search">{search}</div>
      <p className="drawer-count">{t("appManager.count", { count: visible.length })}</p>
      <div className="drawer-list">
        {groups.map(([letter, entries]) => <section className="drawer-group" key={letter}>
          <h2 className="drawer-letter" aria-label={letter}>{letter}</h2>
          {entries.map((app) => <button className="launcher-row" key={app.id} type="button" onClick={() => open(app.path)}>
            <span className={`launcher-row__art${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
            <span className="launcher-row__text"><strong>{app.title}</strong><small>{app.id}</small></span>
            <ArrowIcon />
          </button>)}
        </section>)}
        {visible.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
      </div>
    </div> : null}

    {view === "canvas" ? <div className="launcher-view launcher-view--canvas">
      {search}
      <div className="launcher-canvas" role="list">
        {visible.map((app) => <button className="launcher-card" key={app.id} type="button" role="listitem" onClick={() => open(app.path)}>
          <span className={`launcher-card__art${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
          <strong>{app.title}</strong>
          <small>{app.id}</small>
          <span className="launcher-card__cta">{t("launcher.openApp")}<ArrowIcon /></span>
        </button>)}
      </div>
      {visible.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
    </div> : null}
  </main>;
}
