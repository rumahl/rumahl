import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { AppIcon, AppTile } from "../../apps/AppTile";
import { useShellApps } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useWorkspace } from "../../preferences/Workspace";
import { useShell } from "../ShellContext";
import { useClock } from "../clock";
import { applyOrder, moveInOrder, paginate, setLauncherOrder, useLauncherOrder } from "./launcher-order";

/** Grid: a fixed number of rows and a responsive number of columns. */
const ROWS = 4;
const MIN_CELL = 76;
const GAP = 12;

/**
 * The launcher springboard: a single free surface with multiple pages. The
 * order is stored (not pixel positions) so the grid stays responsive; drag an
 * icon onto another to reorder across pages. Recently used apps and workspace
 * folders sit above the board.
 */
export function LauncherHome() {
  const { t, locale } = useI18n();
  const { snapshot } = useShell();
  const apps = useShellApps();
  const { effective } = useWorkspace();
  const order = useLauncherOrder();
  const [query, setQuery] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [cols, setCols] = useState(4);
  const now = useClock();
  const swipe = useRef<number | null>(null);
  const board = useRef<HTMLDivElement>(null);
  const suppressClick = useRef(false);
  const drag = useRef<null | { start: number; pointer: number; element: HTMLElement; x: number; y: number; moved: boolean }>(null);

  useEffect(() => {
    const host = board.current;
    if (!host) return;
    const measure = () => setCols(Math.max(2, Math.floor((host.clientWidth + GAP) / (MIN_CELL + GAP))));
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  const pageSize = cols * ROWS;
  const byId = useMemo(() => new Map(apps.map((app) => [app.id, app])), [apps]);
  const orderedIds = useMemo(() => applyOrder(apps.map((app) => app.id), order), [apps, order]);
  const normalized = query.trim().toLocaleLowerCase();
  const currentFolder = effective.folders.find((item) => item.id === folder);
  useEffect(() => { setPage(0); }, [query, folder, pageSize]);

  const rootIds = useMemo(
    () => orderedIds.filter((id) => !effective.folders.some((item) => item.apps.includes(id))),
    [orderedIds, effective.folders]
  );
  const pages = useMemo(() => paginate(rootIds, pageSize), [rootIds, pageSize]);
  const activePage = Math.min(page, pages.length - 1);
  const filtered = apps.filter((app) => {
    if (normalized) return `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized);
    if (currentFolder) return currentFolder.apps.includes(app.id);
    return false;
  });

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

  function beginDrag(index: number, event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    const element = event.currentTarget;
    drag.current = { start: index, pointer: event.pointerId, element, x: event.clientX, y: event.clientY, moved: false };
    element.setPointerCapture?.(event.pointerId);
    element.classList.add("is-dragging");
  }
  function moveDrag(event: ReactPointerEvent<HTMLElement>) {
    const entry = drag.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    const dx = event.clientX - entry.x, dy = event.clientY - entry.y;
    if (Math.abs(dx) + Math.abs(dy) > 4) entry.moved = true;
    entry.element.style.transform = `translate(${dx}px, ${dy}px)`;
  }
  function endDrag(event: ReactPointerEvent<HTMLElement>) {
    const entry = drag.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    entry.element.classList.remove("is-dragging");
    entry.element.style.transform = "";
    if (entry.moved) {
      suppressClick.current = true;
      const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-springboard-index]");
      const to = target ? Number(target.dataset.springboardIndex) : -1;
      if (Number.isInteger(to) && to >= 0 && to !== entry.start) {
        const fromId = rootIds[entry.start];
        const toId = rootIds[to];
        const fromOrdered = fromId ? orderedIds.indexOf(fromId) : -1;
        const toOrdered = toId ? orderedIds.indexOf(toId) : -1;
        if (fromOrdered >= 0 && toOrdered >= 0) setLauncherOrder(moveInOrder(orderedIds, fromOrdered, toOrdered));
      }
    }
    drag.current = null;
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

    {normalized || currentFolder ? <section className="launcher-section">
      <h2>{currentFolder ? currentFolder.name : t("launcher.searchResults")}</h2>
      {currentFolder ? <button className="launcher-back" type="button" onClick={() => setFolder(null)}>{t("workspace.back")} · {currentFolder.name}</button> : null}
      <div className="app-grid">{filtered.map((app) => <AppTile app={app} key={app.id} />)}</div>
      {filtered.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
    </section> : <div className="launcher-board" ref={board}
      onPointerDown={onPointerDown} onPointerUp={onPointerUp} onPointerCancel={() => { swipe.current = null; }} onKeyDown={onKeyDown} tabIndex={0}>
      {!normalized && effective.folders.length > 0 ? <section className="launcher-section">
        <h2>{t("launcher.folders")}</h2>
        <div className="app-grid">
          {effective.folders.map((item) => <button className="app-tile" key={item.id} type="button" onClick={() => setFolder(item.id)}>
            <span className="app-tile__icon folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span><span>{item.name}</span>
          </button>)}
        </div>
      </section> : null}
      <div className="springboard-grid" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
        {pages[activePage]!.map((id, localIndex) => {
          const app = byId.get(id);
          if (!app) return null;
          const index = activePage * pageSize + localIndex;
          return <div key={id} className="springboard-cell" data-springboard-index={index}
            onPointerDown={(event) => beginDrag(index, event)} onPointerMove={moveDrag} onPointerUp={endDrag} onPointerCancel={endDrag}
            onDragStart={(event) => event.preventDefault()}
            onClickCapture={(event) => { if (suppressClick.current) { event.preventDefault(); event.stopPropagation(); suppressClick.current = false; } }}>
            <AppTile app={app} />
          </div>;
        })}
      </div>
      {pages.length > 1 ? <nav className="launcher-pages" aria-label={t("launcher.pages")}>
        {pages.map((_, index) => <button key={index} type="button" aria-current={index === activePage ? "page" : undefined}
          aria-label={t("launcher.page", { page: index + 1 })} onClick={() => setPage(index)} />)}
      </nav> : null}
    </div>}
  </main>;
}
