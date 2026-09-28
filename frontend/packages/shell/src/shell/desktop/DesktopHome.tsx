import { useCallback, useMemo, useRef, useState, type DragEvent, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { AppIcon } from "../../apps/AppTile";
import { rememberRecent } from "../../apps/recents";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useShellPreferences } from "../../preferences/ShellPreferences";
import { useWorkspace } from "../../preferences/Workspace";
import { SearchIcon } from "../../icons";
import { ShellLink } from "../../routing/ShellLink";
import { useShell } from "../ShellContext";
import { DesktopContextMenu, type DesktopMenuState } from "./DesktopContextMenu";
import { useDesktopLayout } from "./desktop-layout";
import { DesktopWidgets } from "./DesktopWidgets";
import { rectsIntersect, type SelectionRect } from "./geometry";

export function DesktopHome() {
  const { t } = useI18n();
  const { snapshot, state, dispatch, open } = useShell();
  const settings = useShellPreferences();
  const workspace = useWorkspace();
  const apps = useShellApps();
  const byId = useMemo(() => new Map(apps.map((app) => [app.id, app])), [apps]);
  const layout = useDesktopLayout(apps.map((app) => app.id));
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [dragging, setDragging] = useState<string | null>(null);
  const [menu, setMenu] = useState<DesktopMenuState | null>(null);
  const [marquee, setMarquee] = useState<SelectionRect | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const shortcuts = useRef<HTMLDivElement>(null);
  const marqueeStart = useRef<{ x: number; y: number; base: ReadonlySet<string> } | null>(null);

  const currentFolder = workspace.effective.folders.find((item) => item.id === folder);
  const visible = (currentFolder ? currentFolder.apps.map((id) => byId.get(id)) : layout.visibleIds.map((id) => byId.get(id)))
    .filter((app): app is ShellApp => app !== undefined);
  const hidden = layout.layout.hidden.map((id) => byId.get(id)).filter((app): app is ShellApp => app !== undefined);
  const menuApp = menu?.appId ? byId.get(menu.appId) : undefined;

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

  function onIconClick(app: ShellApp, event: MouseEvent) {
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

  function onDrop(target: string, event: DragEvent) {
    event.preventDefault();
    if (dragging && dragging !== target) layout.move(dragging, target);
    setDragging(null);
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

  return <main className="desktop-home" aria-label={t("desktop.label")} onContextMenu={openShortcutsMenu}>
    <div ref={shortcuts} className="desktop-shortcuts" onPointerDown={beginMarquee} onPointerMove={moveMarquee} onPointerUp={endMarquee} onPointerCancel={endMarquee}>
      <div className="desktop-grid" role="group" aria-label={t("desktop.shortcuts")}>
        {!currentFolder ? workspace.effective.folders.map((item) => <button key={item.id} type="button" className="desktop-icon" onClick={() => { setFolder(item.id); setSelected(new Set()); }}>
          <span className="desktop-icon__art folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span>
          <span className="desktop-icon__label">{item.name}</span>
        </button>) : null}
        {currentFolder ? <button type="button" className="desktop-icon" onClick={() => setFolder(null)}>
          <span className="desktop-icon__art" aria-hidden="true">←</span>
          <span className="desktop-icon__label">{t("workspace.back")}</span>
        </button> : null}
        {visible.map((app) => <ShellLink
          key={app.id}
          to={app.path}
          className={`desktop-icon${selected.has(app.id) ? " is-selected" : ""}${dragging === app.id ? " is-dragging" : ""}`}
          data-app-id={app.id}
          draggable
          aria-pressed={selected.has(app.id)}
          onClick={(event) => onIconClick(app, event)}
          onKeyDown={(event) => onIconKeyDown(app, event)}
          onDragStart={(event) => { setDragging(app.id); event.dataTransfer?.setData("text/plain", app.id); if (event.dataTransfer) event.dataTransfer.effectAllowed = "move"; }}
          onDragEnd={() => setDragging(null)}
          onDragOver={(event) => { if (dragging) event.preventDefault(); }}
          onDrop={(event) => onDrop(app.id, event)}
          onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); setSelected(new Set([app.id])); setMenu({ x: event.clientX, y: event.clientY, appId: app.id }); }}
        >
          <span className={`desktop-icon__art${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
          <span className="desktop-icon__label">{app.title}</span>
        </ShellLink>)}
      </div>
      {marquee ? <span className="desktop-marquee" aria-hidden="true" style={{ left: marquee.x, top: marquee.y, width: marquee.width, height: marquee.height }} /> : null}
      <button className="desktop-search" type="button" onClick={() => dispatch({ type: "toggle-command-palette" })}>
        <SearchIcon />
        <span>{t("search.system")}</span>
        <kbd>⌘ K</kbd>
      </button>
    </div>
    {layout.layout.widgets ? <DesktopWidgets systemStatus={snapshot.systemStatus} /> : null}
    <div className="desktop-wordmark" aria-hidden="true"><span>rumahl</span><small>{t("desktop.tagline")}</small></div>
    <DesktopContextMenu
      menu={menu}
      appTitle={menuApp?.title}
      hidden={hidden}
      widgets={layout.layout.widgets}
      onClose={() => setMenu(null)}
      onOpen={(id) => { const app = byId.get(id); if (app) navigate(app); }}
      onRemove={(id) => { layout.hide(id); setSelected(new Set()); }}
      onRestoreApp={layout.restore}
      onSave={saveArrangement}
      onRestoreLayout={workspace.requestRestore}
      onShowDesktop={() => { dispatch({ type: "minimize-all" }); open("/"); }}
      onResetLayout={layout.reset}
      onToggleWidgets={layout.toggleWidgets}
      onOpenSettings={() => open("/settings")}
    />
  </main>;
}
