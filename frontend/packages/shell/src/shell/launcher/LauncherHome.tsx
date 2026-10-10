import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from "react";
import { createPortal } from "react-dom";
import { AppIcon, AppTile } from "../../apps/AppTile";
import { useShellApps } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useWorkspace } from "../../preferences/Workspace";
import { useShell } from "../ShellContext";
import { useClock } from "../clock";
import { applyOrder, moveInOrder, paginate, setLauncherOrder, useLauncherOrder } from "./launcher-order";
import { LauncherMenu, type LauncherMenuItem } from "./LauncherMenu";

const ROWS = 4;
const MIN_CELL = 76;
const GAP = 12;

interface DragState { id: string; from: number; to: number; x: number; y: number; moved: boolean; width: number }

/**
 * The launcher springboard: a single free surface with pages. Icons can be
 * rearranged only in edit mode (drag onto one another to reorder — a gap opens
 * live — or onto a folder to file them). Recently used apps are not shown
 * separately. Folders open in an overlay.
 */
export function LauncherHome() {
  const { t, locale } = useI18n();
  const { snapshot, open } = useShell();
  const apps = useShellApps();
  const { effective, save } = useWorkspace();
  const order = useLauncherOrder();
  const [query, setQuery] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [page, setPage] = useState(0);
  const [cols, setCols] = useState(4);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [dropFolder, setDropFolder] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [outDrag, setOutDrag] = useState<null | { id: string; x: number; y: number; width: number }>(null);
  const outGesture = useRef<null | { pointer: number; x: number; y: number; width: number }>(null);
  const outDragRef = useRef<null | { folderId: string; appId: string }>(null);
  const [menu, setMenu] = useState<null | { x: number; y: number; appId?: string; folderId?: string }>(null);
  const now = useClock();
  const swipe = useRef<number | null>(null);
  const board = useRef<HTMLDivElement>(null);
  const suppressClick = useRef(false);
  const gesture = useRef<null | { pointer: number; x: number; y: number; to: number; folder: string | null }>(null);

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
  useEffect(() => { setPage(0); }, [query, pageSize]);

  const rootIds = useMemo(
    () => orderedIds.filter((id) => !effective.folders.some((item) => item.apps.includes(id))),
    [orderedIds, effective.folders]
  );
  const boardIds = drag && drag.moved && drag.to !== drag.from && !dropFolder ? moveInOrder(rootIds, drag.from, drag.to) : rootIds;
  const pages = useMemo(() => paginate(boardIds, pageSize), [boardIds, pageSize]);
  const activePage = Math.min(page, pages.length - 1);
  const results = normalized ? apps.filter((app) => `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized)) : [];

  const clock = now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "";
  const date = now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : "";
  const hour = now ? now.getHours() : null;
  const greeting = hour === null ? "launcher.greetingHello"
    : hour < 5 ? "launcher.greetingNight"
    : hour < 12 ? "launcher.greetingMorning"
    : hour < 18 ? "launcher.greetingAfternoon"
    : "launcher.greetingEvening";
  const step = (delta: number) => setPage((value) => Math.min(Math.max(value + delta, 0), pages.length - 1));

  function persistFolders(folders: typeof effective.folders) {
    void save("device", { ...effective, folders });
  }
  function newFolder() {
    const id = `folder-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
    persistFolders([...effective.folders, { id, name: t("launcher.newFolder"), apps: [] }]);
    setFolder(id);
  }
  function removeFolder(id: string) {
    persistFolders(effective.folders.filter((item) => item.id !== id));
    if (folder === id) setFolder(null);
  }
  function fileIntoFolder(folderId: string, appId: string) {
    persistFolders(effective.folders.map((item) => item.id === folderId
      ? { ...item, apps: item.apps.includes(appId) ? item.apps : [...item.apps, appId] }
      : item));
  }
  function removeFromFolder(folderId: string, appId: string) {
    persistFolders(effective.folders.map((item) => item.id === folderId
      ? { ...item, apps: item.apps.filter((id) => id !== appId) }
      : item));
  }
  function renameFolder(folderId: string, name: string) {
    const trimmed = name.trim();
    if (trimmed) persistFolders(effective.folders.map((item) => item.id === folderId ? { ...item, name: trimmed } : item));
    setRenaming(null);
  }
  function beginOutDrag(folderId: string, appId: string, event: ReactPointerEvent<HTMLElement>) {
    if (!editing || event.button !== 0) return;
    const width = event.currentTarget.getBoundingClientRect().width || MIN_CELL;
    outGesture.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, width };
    outDragRef.current = { folderId, appId };
    setOutDrag({ id: appId, x: event.clientX, y: event.clientY, width });
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }
  function moveOutDrag(event: ReactPointerEvent) {
    const entry = outGesture.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    setOutDrag((previous) => previous ? { ...previous, x: event.clientX, y: event.clientY } : previous);
  }
  function endOutDrag(event: ReactPointerEvent) {
    const entry = outGesture.current;
    const target = outDragRef.current;
    if (!entry || entry.pointer !== event.pointerId || !target) return;
    const dx = event.clientX - entry.x, dy = event.clientY - entry.y;
    outGesture.current = null;
    outDragRef.current = null;
    if (Math.abs(dx) + Math.abs(dy) > 6 && !document.elementFromPoint(event.clientX, event.clientY)?.closest(".launcher-folder")) {
      removeFromFolder(target.folderId, target.appId);
    }
    setOutDrag(null);
  }
  function menuItems(): LauncherMenuItem[] {
    if (!menu) return [];
    if (menu.appId) {
      const app = byId.get(menu.appId);
      if (!app) return [];
      return [
        { label: t("launcher.menu.open"), onSelect: () => open(app.path) },
        ...(!app.system ? [{ label: t("launcher.menu.settings"), onSelect: () => open(`${app.path}/settings`) }] : []),
        ...(currentFolder?.apps.includes(app.id) ? [{ label: t("launcher.menu.removeFromFolder"), danger: true, onSelect: () => removeFromFolder(currentFolder.id, app.id) }] : [])
      ];
    }
    if (menu.folderId) {
      const id = menu.folderId;
      return [
        { label: t("launcher.menu.openFolder"), onSelect: () => setFolder(id) },
        { label: t("launcher.menu.renameFolder"), onSelect: () => setRenaming(id) },
        { label: t("launcher.menu.removeFolder"), danger: true, onSelect: () => removeFolder(id) }
      ];
    }
    return [{ label: t("launcher.menu.newFolder"), onSelect: newFolder }];
  }
  function openMenu(event: ReactMouseEvent, extra: { appId?: string; folderId?: string } = {}) {
    event.preventDefault();
    event.stopPropagation();
    setMenu({ x: event.clientX, y: event.clientY, ...extra });
  }

  function onPointerDown(event: ReactPointerEvent) {
    if (event.pointerType === "mouse") return;
    swipe.current = event.clientX;
  }
  function onPointerUp(event: ReactPointerEvent) {
    const start = swipe.current;
    swipe.current = null;
    if (start === null || drag || currentFolder) return;
    const distance = event.clientX - start;
    if (Math.abs(distance) >= 60) step(distance < 0 ? 1 : -1);
  }
  function onKeyDown(event: ReactKeyboardEvent) {
    if ((event.target as HTMLElement).closest("input, textarea, select")) return;
    if (event.key === "ArrowRight") { event.preventDefault(); step(1); }
    else if (event.key === "ArrowLeft") { event.preventDefault(); step(-1); }
    else if (event.key === "Escape" && currentFolder) { event.preventDefault(); setFolder(null); }
  }

  function beginDrag(index: number, id: string, event: ReactPointerEvent<HTMLElement>) {
    if (!editing || event.button !== 0) return;
    const cell = event.currentTarget.closest<HTMLElement>(".springboard-cell");
    const width = cell?.getBoundingClientRect().width ?? MIN_CELL;
    gesture.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, to: index, folder: null };
    setDrag({ id, from: index, to: index, x: event.clientX, y: event.clientY, moved: false, width });
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }
  function moveDrag(event: ReactPointerEvent) {
    const entry = gesture.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    const dx = event.clientX - entry.x, dy = event.clientY - entry.y;
    if (Math.abs(dx) + Math.abs(dy) > 6) {
      const element = document.elementFromPoint(event.clientX, event.clientY);
      const folderEl = element?.closest<HTMLElement>("[data-folder-id]");
      const cellEl = element?.closest<HTMLElement>("[data-springboard-index]");
      const nextFolder = folderEl?.dataset.folderId ?? null;
      const nextTo = !nextFolder && cellEl ? Number(cellEl.dataset.springboardIndex) : entry.to;
      entry.folder = nextFolder;
      if (Number.isInteger(nextTo)) entry.to = nextTo;
      setDropFolder(nextFolder);
      setDrag((previous) => previous ? { ...previous, moved: true, to: entry.to, x: event.clientX, y: event.clientY } : previous);
    } else {
      setDrag((previous) => previous ? { ...previous, x: event.clientX, y: event.clientY } : previous);
    }
  }
  function endDrag(event: ReactPointerEvent) {
    const entry = gesture.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    gesture.current = null;
    if (drag?.moved) {
      suppressClick.current = true;
      if (entry.folder) fileIntoFolder(entry.folder, drag.id);
      else if (entry.to !== drag.from) setLauncherOrder(moveInOrder(rootIds, drag.from, entry.to));
    }
    setDrag(null);
    setDropFolder(null);
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
      {!normalized && !currentFolder ? <button className="launcher-edit" type="button" aria-pressed={editing} onClick={() => setEditing((value) => !value)}>
        {t(editing ? "launcher.done" : "launcher.edit")}
      </button> : null}
    </header>

    {normalized ? <section className="launcher-section">
      <h2>{t("launcher.searchResults")}</h2>
      <div className="app-grid">{results.map((app) => <AppTile app={app} key={app.id} />)}</div>
      {results.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.noResults")}</p> : null}
    </section> : <div className={`launcher-board${editing ? " is-editing" : ""}`} ref={board} onContextMenu={(event) => openMenu(event)}
      onPointerDown={onPointerDown} onPointerUp={onPointerUp} onPointerCancel={() => { swipe.current = null; }} onKeyDown={onKeyDown} tabIndex={0}>
      {effective.folders.length > 0 ? <section className="launcher-section launcher-section--folders">
        <h2>{t("launcher.folders")}</h2>
        <div className="app-grid">
          {effective.folders.map((item) => renaming === item.id
            ? <div className="app-tile" key={item.id} data-folder-id={item.id}>
                <span className="app-tile__icon folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span>
                <input className="launcher-rename" autoFocus defaultValue={item.name} maxLength={128}
                  onClick={(event) => event.stopPropagation()}
                  onBlur={(event) => renameFolder(item.id, event.target.value)}
                  onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); if (event.key === "Escape") setRenaming(null); }} />
              </div>
            : <button className={`app-tile${dropFolder === item.id ? " is-drop-target" : ""}`} key={item.id} type="button"
                data-folder-id={item.id} onClick={() => setFolder(item.id)} onContextMenu={(event) => openMenu(event, { folderId: item.id })}>
                <span className="app-tile__icon folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span><span>{item.name}</span>
              </button>)}
        </div>
      </section> : null}
      <div className="springboard-grid" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
        {pages[activePage]!.map((id, localIndex) => {
          const app = byId.get(id);
          if (!app) return null;
          const index = activePage * pageSize + localIndex;
          const placeholder = drag?.moved && drag.id === id && !dropFolder;
          return <div key={id} className={`springboard-cell${placeholder ? " is-placeholder" : ""}`} data-springboard-index={index}
            onPointerDown={(event) => beginDrag(index, app.id, event)} onPointerMove={moveDrag} onPointerUp={endDrag} onPointerCancel={endDrag}
            onDragStart={(event) => event.preventDefault()}
            onContextMenu={(event) => openMenu(event, { appId: app.id })}
            onClickCapture={(event) => { if (suppressClick.current) { event.preventDefault(); event.stopPropagation(); suppressClick.current = false; } }}>
            {placeholder ? null : <AppTile app={app} />}
          </div>;
        })}
      </div>
      {pages.length > 1 ? <nav className="launcher-pages" aria-label={t("launcher.pages")}>
        {pages.map((_, index) => <button key={index} type="button" aria-current={index === activePage ? "page" : undefined}
          aria-label={t("launcher.page", { page: index + 1 })} onClick={() => setPage(index)} />)}
      </nav> : null}
    </div>}

    {currentFolder ? <div className="launcher-folder-overlay" role="dialog" aria-label={currentFolder.name} onClick={(event) => { if (event.target === event.currentTarget) setFolder(null); }}>
      <div className="launcher-folder">
        <header className="launcher-folder__header">
          {renaming === currentFolder.id
            ? <input className="launcher-rename" autoFocus defaultValue={currentFolder.name} maxLength={128}
                onBlur={(event) => renameFolder(currentFolder.id, event.target.value)}
                onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); if (event.key === "Escape") setRenaming(null); }} />
            : <h2>{currentFolder.name}</h2>}
          <div className="launcher-folder__actions">
            <button className="launcher-edit" type="button" aria-pressed={editing} onClick={() => setEditing((value) => !value)}>{t(editing ? "launcher.done" : "launcher.edit")}</button>
            <button type="button" aria-label={t("launcher.closeFolder")} onClick={() => setFolder(null)}>×</button>
          </div>
        </header>
        <div className="app-grid">
          {currentFolder.apps.map((id) => { const app = byId.get(id); return app ? <div key={id}
            onContextMenu={(event) => openMenu(event, { appId: id })}
            onPointerDown={(event) => beginOutDrag(currentFolder.id, id, event)} onPointerMove={moveOutDrag} onPointerUp={endOutDrag} onPointerCancel={endOutDrag}>
            <AppTile app={app} /></div> : null; })}
        </div>
        {currentFolder.apps.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.folderEmpty")}</p> : null}
      </div>
    </div> : null}

    {outDrag ? createPortal(
      <div className="springboard-float" style={{ left: outDrag.x, top: outDrag.y, width: outDrag.width }} aria-hidden="true">
        {byId.get(outDrag.id) ? <AppTile app={byId.get(outDrag.id)!} /> : null}
      </div>, document.body) : null}

    {drag?.moved && !dropFolder ? createPortal(
      <div className="springboard-float" style={{ left: drag.x, top: drag.y, width: drag.width }} aria-hidden="true">
        {byId.get(drag.id) ? <AppTile app={byId.get(drag.id)!} /> : null}
      </div>, document.body) : null}
    {menu ? <LauncherMenu x={menu.x} y={menu.y} items={menuItems()} onClose={() => setMenu(null)} /> : null}
  </main>;
}
