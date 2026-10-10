import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { AppIcon } from "../../apps/AppTile";
import { rememberRecent } from "../../apps/recents";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useShellPreferences } from "../../preferences/ShellPreferences";
import { useWorkspace } from "../../preferences/Workspace";
import { setDesktopPosition, useDesktopPositions, clampDesktopPosition, desktopPositionInBounds, getDesktopPositions } from "./desktop-positions";
import { ShellLink } from "../../routing/ShellLink";
import { useShell } from "../ShellContext";
import { DesktopContextMenu, type DesktopMenuState } from "./DesktopContextMenu";
import { useDesktopLayout } from "./desktop-layout";
import { DesktopWidgets } from "./DesktopWidgets";
import { rectsIntersect, type SelectionRect } from "./geometry";

/** Nominal icon footprint used to keep free positions inside the container. */
const ICON_FOOTPRINT = { width: 96, height: 104 };

export function DesktopHome() {
  const { t } = useI18n();
  const { snapshot, state, dispatch, open } = useShell();
  const settings = useShellPreferences();
  const workspace = useWorkspace();
  const apps = useShellApps();
  const byId = useMemo(() => new Map(apps.map((app) => [app.id, app])), [apps]);
  const layout = useDesktopLayout(apps.map((app) => app.id));
  const positions = useDesktopPositions();
  const suppressClick = useRef(false);
  const freeDrag = useRef<null | { id: string; pointer: number; startX: number; startY: number; originX: number; originY: number; element: HTMLElement; host: HTMLElement; moved: boolean }>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<DesktopMenuState | null>(null);
  const [marquee, setMarquee] = useState<SelectionRect | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const shortcuts = useRef<HTMLDivElement>(null);
  const marqueeStart = useRef<{ x: number; y: number; base: ReadonlySet<string> } | null>(null);

  // Repair stale or dragged-off-screen free positions (and keep them in bounds
  // on resize) so an icon never hides past the container edge.
  useEffect(() => {
    const host = shortcuts.current;
    if (!host) return;
    const sanitize = () => {
      const bounds = { width: host.clientWidth, height: host.clientHeight };
      if (!bounds.width || !bounds.height) return;
      for (const [id, position] of Object.entries(getDesktopPositions())) {
        if (!desktopPositionInBounds(position, bounds, ICON_FOOTPRINT)) {
          setDesktopPosition(id, clampDesktopPosition(position, bounds, ICON_FOOTPRINT));
        }
      }
    };
    sanitize();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(sanitize);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

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
    const element = event.currentTarget as HTMLElement;
    if (!host) return;
    const box = host.getBoundingClientRect();
    const icon = element.getBoundingClientRect();
    // Add the scroll offset: `getBoundingClientRect` is viewport-relative while
    // an absolute position is relative to the (unscrolled) container.
    const origin = positions[app.id] ?? { x: icon.left - box.left + host.scrollLeft, y: icon.top - box.top + host.scrollTop };
    element.style.position = "absolute";
    element.style.left = `${origin.x}px`;
    element.style.top = `${origin.y}px`;
    element.style.zIndex = "5";
    element.setPointerCapture(event.pointerId);
    freeDrag.current = { id: app.id, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, originX: origin.x, originY: origin.y, element, host, moved: false };
  }

  function moveFreeDrag(event: PointerEvent<HTMLElement>) {
    const drag = freeDrag.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    const dx = event.clientX - drag.startX, dy = event.clientY - drag.startY;
    if (Math.abs(dx) + Math.abs(dy) > 4) drag.moved = true;
    drag.element.style.left = `${drag.originX + dx}px`;
    drag.element.style.top = `${drag.originY + dy}px`;
  }

  function endFreeDrag(event: PointerEvent<HTMLElement>) {
    const drag = freeDrag.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    const dx = event.clientX - drag.startX, dy = event.clientY - drag.startY;
    drag.element.style.zIndex = "";
    if (drag.moved) {
      suppressClick.current = true;
      const bounds = { width: drag.host.clientWidth, height: drag.host.clientHeight };
      const footprint = { width: drag.element.offsetWidth || ICON_FOOTPRINT.width, height: drag.element.offsetHeight || ICON_FOOTPRINT.height };
      setDesktopPosition(drag.id, clampDesktopPosition({ x: drag.originX + dx, y: drag.originY + dy }, bounds, footprint));
    } else {
      drag.element.style.position = "";
      drag.element.style.left = "";
      drag.element.style.top = "";
    }
    freeDrag.current = null;
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
        {!currentFolder ? workspace.effective.folders.map((item) => <button key={item.id} type="button" className="desktop-icon desktop-app" onClick={() => { setFolder(item.id); setSelected(new Set()); }}>
          <span className="desktop-icon__art folder-preview" aria-hidden="true">{item.apps.slice(0, 4).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span>
          <span className="desktop-icon__label">{item.name}</span>
        </button>) : null}
        {currentFolder ? <button type="button" className="desktop-icon desktop-app" onClick={() => setFolder(null)}>
          <span className="desktop-icon__art" aria-hidden="true">←</span>
          <span className="desktop-icon__label">{t("workspace.back")}</span>
        </button> : null}
        {visible.map((app) => {
          const position = positions[app.id];
          return <ShellLink
            key={app.id}
            to={app.path}
            className={`desktop-icon desktop-app${selected.has(app.id) ? " is-selected" : ""}`}
            data-app-id={app.id}
            aria-pressed={selected.has(app.id)}
            style={position ? { position: "absolute", left: position.x, top: position.y } : undefined}
            onClick={(event) => onIconClick(app, event)}
            onKeyDown={(event) => onIconKeyDown(app, event)}
            onPointerDown={(event) => beginFreeDrag(app, event)}
            onPointerMove={moveFreeDrag}
            onPointerUp={endFreeDrag}
            onPointerCancel={endFreeDrag}
            onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); setSelected(new Set([app.id])); setMenu({ x: event.clientX, y: event.clientY, appId: app.id }); }}
          >
            <span className={`desktop-icon__art${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
            <span className="desktop-icon__label">{app.title}</span>
          </ShellLink>;
        })}
      </div>
      {marquee ? <span className="desktop-marquee" aria-hidden="true" style={{ left: marquee.x, top: marquee.y, width: marquee.width, height: marquee.height }} /> : null}
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
