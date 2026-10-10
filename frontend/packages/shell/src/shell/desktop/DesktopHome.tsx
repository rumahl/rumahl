import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent } from "react";
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
/** Gap between desktop grid cells (matches `.desktop-grid`). */
const ICON_GAP = 6;

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
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<DesktopMenuState | null>(null);
  const [marquee, setMarquee] = useState<SelectionRect | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [dragIcon, setDragIcon] = useState<null | { id: string; x: number; y: number }>(null);
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

  // A drag suppresses the click that follows its release; any *new* pointer
  // gesture (even outside an icon) must clear that flag so the next click is
  // not swallowed.
  useEffect(() => {
    const clear = () => { suppressClick.current = false; };
    window.addEventListener("pointerdown", clear, true);
    return () => window.removeEventListener("pointerdown", clear, true);
  }, []);

  const currentFolder = workspace.effective.folders.find((item) => item.id === folder);
  const visible = (currentFolder ? currentFolder.apps.map((id) => byId.get(id)) : layout.visibleIds.map((id) => byId.get(id)))
    .filter((app): app is ShellApp => app !== undefined);
  const hidden = layout.layout.hidden.map((id) => byId.get(id)).filter((app): app is ShellApp => app !== undefined);
  const menuApp = menu?.appId ? byId.get(menu.appId) : undefined;

  // Apply free positions with CSSOM: the shell's strict style-src CSP blocks an
  // inline `style` attribute (React's `style` prop), so positions must be set
  // through `element.style` — like `ProtectedWindow` does for windows.
  useLayoutEffect(() => {
    const host = shortcuts.current;
    if (!host) return;
    for (const element of host.querySelectorAll<HTMLElement>("[data-app-id]")) {
      const id = element.dataset.appId;
      if (!id) continue;
      const dragging = dragIcon?.id === id ? dragIcon : null;
      const position = dragging ?? positions[id];
      if (position) {
        element.style.position = "absolute";
        element.style.left = `${position.x}px`;
        element.style.top = `${position.y}px`;
        element.style.zIndex = dragging ? "5" : "";
      } else {
        element.style.removeProperty("position");
        element.style.removeProperty("left");
        element.style.removeProperty("top");
        element.style.removeProperty("z-index");
      }
    }
  }, [visible, positions, dragIcon]);

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
        const dx = clientX - startX, dy = clientY - startY;
        const bounds = { width: host.clientWidth, height: host.clientHeight };
        const footprint = { width: element.offsetWidth || ICON_FOOTPRINT.width, height: element.offsetHeight || ICON_FOOTPRINT.height };
        const stepX = ICON_FOOTPRINT.width + ICON_GAP, stepY = ICON_FOOTPRINT.height + ICON_GAP;
        const snapped = { x: Math.round((origin.x + dx) / stepX) * stepX, y: Math.round((origin.y + dy) / stepY) * stepY };
        setDesktopPosition(app.id, clampDesktopPosition(snapped, bounds, footprint));
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

  return <main className="desktop-home" aria-label={t("desktop.label")} onContextMenu={openShortcutsMenu}>
    <div ref={shortcuts} className="desktop-shortcuts" onPointerDown={beginMarquee} onPointerMove={moveMarquee} onPointerUp={endMarquee} onPointerCancel={endMarquee}>
      <div className="desktop-grid" role="group" aria-label={t("desktop.shortcuts")}>
        {!currentFolder ? workspace.effective.folders.map((item) => <button key={item.id} type="button" className="desktop-icon desktop-app" onClick={() => { setFolder(item.id); setSelected(new Set()); }}>
          <span className="desktop-icon__art folder-preview" aria-hidden="true">{item.apps.slice(0, 9).map((id) => { const app = byId.get(id); return app ? <span key={id}><AppIcon app={app} /></span> : null; })}</span>
          <span className="desktop-icon__label">{item.name}</span>
        </button>) : null}
        {currentFolder ? <button type="button" className="desktop-icon desktop-app" onClick={() => setFolder(null)}>
          <span className="desktop-icon__art" aria-hidden="true">←</span>
          <span className="desktop-icon__label">{t("workspace.back")}</span>
        </button> : null}
        {visible.map((app) => {
          return <ShellLink
            key={app.id}
            to={app.path}
            className={`desktop-icon desktop-app${selected.has(app.id) ? " is-selected" : ""}`}
            data-app-id={app.id}
            aria-pressed={selected.has(app.id)}
            draggable={false}
            onClick={(event) => onIconClick(app, event)}
            onKeyDown={(event) => onIconKeyDown(app, event)}
            onPointerDown={(event) => beginFreeDrag(app, event)}
            onDragStart={(event) => event.preventDefault()}
            onContextMenu={(event) => { event.preventDefault(); event.stopPropagation(); setSelected(new Set([app.id])); setMenu({ x: event.clientX, y: event.clientY, appId: app.id }); }}
          >
            <span className={`desktop-icon__art${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
            <span className="desktop-icon__label">{app.title}</span>
          </ShellLink>;
        })}
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
