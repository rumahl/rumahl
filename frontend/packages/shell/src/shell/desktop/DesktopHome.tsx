import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { createPortal } from "react-dom";
import { AppIcon, AppTile } from "../../apps/AppTile";
import { rememberRecent } from "../../apps/recents";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useShellPreferences } from "../../preferences/ShellPreferences";
import { useWorkspace } from "../../preferences/Workspace";
import { ShellLink } from "../../routing/ShellLink";
import { useShell } from "../ShellContext";
import { useDesktopFiles, type DesktopFile } from "../desktop-files";
import { useSafeMode } from "../../preferences/safe-mode";
import { FileIcon, FolderIcon } from "../../apps/files/icons";
import { DesktopContextMenu, type DesktopMenuState } from "./DesktopContextMenu";
import { useDesktopLayout } from "./desktop-layout";
import { DesktopWidgets } from "./DesktopWidgets";
import { createCellStore, gridMetrics, nearestFreeCell, resolveCells, useGridSize, type GridCell } from "./free-grid";
import { rectsIntersect, type SelectionRect } from "./geometry";

/** Nominal grid cell (content + gap) used by the free desktop grid. */
const CELL = { width: 96, height: 106 };
const GAP = 6;
/** Space reserved for the dock so app icons never sit at dock height. */
const DOCK_RESERVE = 94;

/** Grid cells of every desktop item; the layout adapts to the container size. */
export const desktopCells = createCellStore("rumahl.desktop.cells");

interface DesktopItem { id: string; kind: "folder" | "app" | "file"; app?: ShellApp; folderId?: string; name: string; file?: DesktopFile }

export function DesktopHome() {
  const { t } = useI18n();
  const { snapshot, state, dispatch, open, live } = useShell();
  const settings = useShellPreferences();
  const workspace = useWorkspace();
  const allApps = useShellApps();
  const safeMode = useSafeMode();
  const apps = useMemo(() => (safeMode ? allApps.filter((app) => app.system) : allApps), [allApps, safeMode]);
  const byId = useMemo(() => new Map(apps.map((app) => [app.id, app])), [apps]);
  const layout = useDesktopLayout(apps.map((app) => app.id));
  const storedCells = desktopCells.use();
  const desktopFiles = useDesktopFiles(live?.request ?? null);
  const suppressClick = useRef(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<DesktopMenuState | null>(null);
  const [marquee, setMarquee] = useState<SelectionRect | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [dragIcon, setDragIcon] = useState<null | { id: string; x: number; y: number }>(null);
  const shortcuts = useRef<HTMLDivElement>(null);
  const marqueeStart = useRef<{ x: number; y: number; base: ReadonlySet<string> } | null>(null);
  const size = useGridSize(shortcuts);
  // The selection surface reaches the bottom edge; only the icon grid keeps the
  // dock reserve so shortcuts cannot be placed under the dock.
  const availableHeight = Math.max(0, size.height - DOCK_RESERVE);
  const cols = useMemo(() => gridMetrics(size.width, availableHeight, CELL.width, CELL.height, GAP).cols, [size.width, availableHeight]);

  // A drag suppresses the click that follows its release; any *new* pointer
  // gesture (even outside an icon) must clear that flag so the next click is
  // not swallowed.
  useEffect(() => {
    const clear = () => { suppressClick.current = false; };
    window.addEventListener("pointerdown", clear, true);
    return () => window.removeEventListener("pointerdown", clear, true);
  }, []);

  const currentFolder = workspace.effective.folders.find((item) => item.id === folder);
  // Apps filed into an app folder live only inside it (as on the launcher).
  const filedAppIds = useMemo(() => new Set(workspace.effective.folders.flatMap((entry) => entry.apps)), [workspace.effective.folders]);
  const visible = layout.visibleIds.map((id) => byId.get(id)).filter((app): app is ShellApp => app !== undefined).filter((app) => !filedAppIds.has(app.id));
  const hidden = layout.layout.hidden.map((id) => byId.get(id)).filter((app): app is ShellApp => app !== undefined);
  const menuApp = menu?.appId ? byId.get(menu.appId) : undefined;

  const items = useMemo<DesktopItem[]>(() => {
    const list: DesktopItem[] = [];
    for (const entry of workspace.effective.folders) list.push({ id: `folder:${entry.id}`, kind: "folder", folderId: entry.id, name: entry.name });
    for (const app of visible) list.push({ id: app.id, kind: "app", app, name: app.title });
    for (const file of desktopFiles.files) list.push({ id: `file:${file.id}`, kind: "file", name: file.name, file });
    return list;
  }, [workspace.effective.folders, visible, desktopFiles.files]);
  // Enough rows to hold every item (never overlap); grows/scrolls if needed.
  const rows = useMemo(() => Math.max(gridMetrics(size.width, availableHeight, CELL.width, CELL.height, GAP).rows, Math.ceil(items.length / Math.max(1, cols))), [size.width, availableHeight, items.length, cols]);
  // Column-major like the macOS desktop: fill top-to-bottom, then the next column.
  const effective = useMemo(() => resolveCells(items.map((item) => item.id), storedCells, cols, rows, "column"), [items, storedCells, cols, rows]);

  // Close the opened folder with Escape.
  useEffect(() => {
    if (!currentFolder) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") setFolder(null); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [currentFolder]);

  // Position every item with CSSOM: the shell's strict style-src CSP blocks an
  // inline `style` attribute, so positions are set through `element.style`.
  useLayoutEffect(() => {
    const host = shortcuts.current;
    if (!host) return;
    const stepX = CELL.width + GAP, stepY = CELL.height + GAP;
    for (const element of host.querySelectorAll<HTMLElement>("[data-grid-id]")) {
      const id = element.dataset.gridId;
      if (!id) continue;
      const dragging = dragIcon?.id === id ? dragIcon : null;
      const cell: GridCell | undefined = effective[id];
      const point = dragging ?? (cell ? { x: cell.col * stepX, y: cell.row * stepY } : { x: 0, y: 0 });
      element.style.position = "absolute";
      element.style.left = `${point.x}px`;
      element.style.top = `${point.y}px`;
      element.style.zIndex = dragging ? "5" : "";
    }
  }, [items, effective, dragIcon]);

  const toggleSelected = useCallback((id: string, additive: boolean) => {
    setSelected((previous) => {
      if (!additive) return new Set([id]);
      const next = new Set(previous);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, []);

  function navigate(app: ShellApp) {
    rememberRecent(app.path);
    open(app.path);
  }

  function openFile(file: DesktopFile) {
    if (file.directory) { open("/app/files"); return; }
    const link = document.createElement("a");
    link.href = `/api/v1/files/content?id=${encodeURIComponent(file.id)}`;
    link.download = file.name;
    link.click();
  }

  function onIconClick(app: ShellApp, event: MouseEvent) {
    if (suppressClick.current) { suppressClick.current = false; event.preventDefault(); return; }
    if (event.metaKey || event.ctrlKey || event.shiftKey) {
      event.preventDefault();
      toggleSelected(app.id, true);
      return;
    }
    rememberRecent(app.path);
    setSelected(new Set([app.id]));
  }

  function onIconKeyDown(app: ShellApp, event: ReactKeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (event.metaKey || event.ctrlKey) { event.preventDefault(); toggleSelected(app.id, true); }
  }

  function beginMarquee(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || (event.target as HTMLElement).closest("[data-app-id], button")) return;
    const additive = event.metaKey || event.ctrlKey || event.shiftKey;
    marqueeStart.current = { x: event.clientX, y: event.clientY, base: additive ? new Set(selected) : new Set() };
    if (!additive) setSelected(new Set());
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function moveMarquee(event: PointerEvent<HTMLDivElement>) {
    const start = marqueeStart.current;
    const host = shortcuts.current;
    if (!start || !host) return;
    const box = host.getBoundingClientRect();
    const left = Math.min(start.x, event.clientX), top = Math.min(start.y, event.clientY);
    const width = Math.abs(event.clientX - start.x), height = Math.abs(event.clientY - start.y);
    if (width < 6 && height < 6) return;
    const rect: SelectionRect = { x: left - box.left, y: top - box.top, width, height };
    setMarquee(rect);
    const next = new Set(start.base);
    host.querySelectorAll<HTMLElement>("[data-app-id]").forEach((element) => {
      const id = element.dataset.appId;
      if (!id) return;
      const icon = element.getBoundingClientRect();
      if (rectsIntersect(rect, { x: icon.left - box.left, y: icon.top - box.top, width: icon.width, height: icon.height })) next.add(id);
    });
    setSelected(next);
  }

  function endMarquee() {
    marqueeStart.current = null;
    setMarquee(null);
  }

  function beginFreeDrag(app: ShellApp, event: PointerEvent<HTMLElement>) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey) return;
    const host = shortcuts.current;
    if (!host) return;
    const stepX = CELL.width + GAP, stepY = CELL.height + GAP;
    const cell = effective[app.id] ?? { col: 0, row: 0 };
    const origin = { x: cell.col * stepX, y: cell.row * stepY };
    const startX = event.clientX, startY = event.clientY;
    let moved = false;
    setDragIcon({ id: app.id, x: origin.x, y: origin.y });
    // Listen on the window: pointer capture can fail for synthetic pointers, so
    // the drag must not depend on the events reaching the icon again.
    const onMove = (move: Event) => {
      const { clientX, clientY } = move as unknown as { clientX: number; clientY: number };
      const dx = clientX - startX, dy = clientY - startY;
      if (Math.abs(dx) + Math.abs(dy) > 4) moved = true;
      if (moved) setDragIcon({ id: app.id, x: origin.x + dx, y: origin.y + dy });
    };
    const onUp = (up: Event) => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      if (moved) {
        suppressClick.current = true;
        const { clientX, clientY } = up as unknown as { clientX: number; clientY: number };
        const target = { col: Math.round((origin.x + (clientX - startX)) / stepX), row: Math.round((origin.y + (clientY - startY)) / stepY) };
        const taken = items.filter((item) => item.id !== app.id).map((item) => effective[item.id]).filter((value): value is GridCell => value !== undefined);
        desktopCells.set(app.id, nearestFreeCell(target, taken, cols, rows));
      }
      setDragIcon(null);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function openShortcutsMenu(event: MouseEvent) {
    event.preventDefault();
    setMenu({ x: event.clientX, y: event.clientY });
  }

  function saveArrangement() {
    void workspace.save(settings.scope, {
      ...workspace.effective,
      windows: state.windows.filter((w) => w.location && !w.streamId).slice(0, 32).map((w) => ({
        location: w.location!, rect: w.rect ?? { x: 36, y: 24, width: 760, height: 540 }, placement: w.placement ?? "floating", minimized: w.minimized
      }))
    });
  }

  return <main className={`desktop-home${currentFolder ? " is-folder-open" : ""}`} aria-label={t("desktop.label")} onContextMenu={openShortcutsMenu}
    onDragOver={(event) => { if (Array.from(event.dataTransfer.types).includes("Files")) event.preventDefault(); }}
    onDrop={(event) => { const dropped = event.dataTransfer?.files; if (!dropped || dropped.length === 0) return; event.preventDefault(); for (const file of Array.from(dropped)) void desktopFiles.upload(file); }}>
    <div ref={shortcuts} className="desktop-shortcuts" onPointerDown={beginMarquee} onPointerMove={moveMarquee} onPointerUp={endMarquee} onPointerCancel={endMarquee}>
      <div className="desktop-grid" role="group" aria-label={t("desktop.shortcuts")}>
        {items.map((item) => item.kind === "folder"
          ? <button key={item.id} type="button" className="desktop-icon desktop-app" data-grid-id={item.id} onClick={() => { setFolder(item.folderId!); setSelected(new Set()); }}>
            <span className="desktop-icon__art folder-preview" aria-hidden="true">{(workspace.effective.folders.find((entry) => entry.id === item.folderId)?.apps ?? []).slice(0, 9).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span>
            <span className="desktop-icon__label" data-adaptive-text="">{item.name}</span>
          </button>
          : item.kind === "file"
          ? <button key={item.id} type="button" className="desktop-icon desktop-app" data-grid-id={item.id}
              onDoubleClick={() => openFile(item.file!)}
              onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); }}>
              <span className="desktop-icon__art is-file" aria-hidden="true">{item.file!.directory ? <FolderIcon /> : <FileIcon />}</span>
              <span className="desktop-icon__label" data-adaptive-text="">{item.name}</span>
            </button>
          : <ShellLink
            key={item.id}
            to={item.app!.path}
            className={`desktop-icon desktop-app${selected.has(item.id) ? " is-selected" : ""}`}
            data-grid-id={item.id}
            data-app-id={item.id}
            aria-pressed={selected.has(item.id)}
            draggable={false}
            onClick={(event) => onIconClick(item.app!, event)}
            onKeyDown={(event) => onIconKeyDown(item.app!, event)}
            onPointerDown={(event) => beginFreeDrag(item.app!, event)}
            onDragStart={(event) => event.preventDefault()}
            onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); setSelected(new Set([item.id])); setMenu({ x: event.clientX, y: event.clientY, appId: item.id }); }}
          >
            <span className={`desktop-icon__art${item.app!.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={item.app!} /></span>
            <span className="desktop-icon__label" data-adaptive-text="">{item.app!.title}</span>
          </ShellLink>)}
      </div>
      {marquee ? <span className="desktop-marquee" aria-hidden="true" ref={(element) => {
        if (!element) return;
        element.style.left = `${marquee.x}px`;
        element.style.top = `${marquee.y}px`;
        element.style.width = `${marquee.width}px`;
        element.style.height = `${marquee.height}px`;
      }} /> : null}
    </div>
    {layout.layout.widgets ? <DesktopWidgets systemStatus={snapshot.systemStatus} /> : null}
    <div className="desktop-wordmark" aria-hidden="true" data-adaptive-text=""><span>rumahl</span><small>{t("desktop.tagline")}</small></div>
    <DesktopContextMenu
      menu={menu}
      appTitle={menuApp?.title}
      appSettingsPath={menuApp?.managePath ?? menuApp?.settingsPath}
      hidden={hidden}
      widgets={layout.layout.widgets}
      onClose={() => setMenu(null)}
      onOpen={(id) => { const app = byId.get(id); if (app) navigate(app); }}
      onOpenAppSettings={(path) => open(path)}
      onRemove={(id) => { layout.hide(id); setSelected(new Set()); }}
      onRestoreApp={layout.restore}
      onSave={saveArrangement}
      onRestoreLayout={workspace.requestRestore}
      onShowDesktop={() => { dispatch({ type: "minimize-all" }); open("/"); }}
      onResetLayout={layout.reset}
      onToggleWidgets={layout.toggleWidgets}
      onOpenSettings={() => open("/settings")}
    />
    {currentFolder ? createPortal(
      <div className="launcher-folder-overlay" role="dialog" aria-label={currentFolder.name}
        onContextMenu={(event) => event.preventDefault()}
        onClick={(event) => { if (event.target === event.currentTarget) setFolder(null); }}>
        <div className="launcher-folder">
          <header className="launcher-folder__header">
            <h2>{currentFolder.name}</h2>
            <div className="launcher-folder__actions">
              <button type="button" aria-label={t("workspace.back")} onClick={() => setFolder(null)}>×</button>
            </div>
          </header>
          <div className="app-grid">
            {currentFolder.apps.map((id) => { const app = byId.get(id); return app ? <div key={id}><AppTile app={app} /></div> : null; })}
          </div>
          {currentFolder.apps.length === 0 ? <p role="status" className="launcher-empty">{t("launcher.folderEmpty")}</p> : null}
        </div>
      </div>, document.body) : null}
  </main>;
}
