import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useLocation } from "react-router";
import { useTheme } from "@rumahl/ui";
import { MobileNavSlotContext, type MobileNavModel } from "@rumahl/ui/components";
import { MobileAppNav } from "./MobileAppNav";
import { ProtectedWindow } from "../components/ProtectedWindow";
import { findFirstPartyApp } from "../apps/registry";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { useI18n } from "../i18n";
import { WindowExitFullscreenIcon } from "../icons";
import { RouteBoundary } from "../routing/RouteBoundary";
import { useShell } from "./ShellContext";
import { useWorkspace } from "../preferences/Workspace";
import type { ShellWindow } from "../shell-state";
import { centeredRect, constrainRect, placedRect, type WindowPlacement, type WindowRect, type WorkArea } from "./desktop/geometry";

import { readWindowRect, writeWindowRect } from "./window-positions";

/** Height of the flush top bar maximized/snapped windows sit below. */
const TOPBAR_HEIGHT = 54;
/** Desktop windows keep a small breathing room from the screen edges. */
const WINDOW_GAP = 12;

/**
 * True for an installed (sandboxed iframe) app window. First-party React apps,
 * streams and pages are not flush.
 */
export function isInstalledAppWindow(id: string): boolean {
  const appId = id.startsWith("app:") ? id.slice("app:".length) : undefined;
  return appId !== undefined && findFirstPartyApp(appId) === undefined;
}

export function DesktopWindows({ animations = true }: { animations?: boolean } = {}) {
  const { state, mode } = useShell();
  const workspace = useWorkspace();
  const layer = useRef<HTMLDivElement>(null);
  const chrome = workspace.userWorkspace?.fullscreenChrome ?? workspace.effective.fullscreenChrome;
  const clearance = mode === "desktop" && chrome.hideDock === false && chrome.dockClearance === true;
  // Space reserved at the bottom of the window layer so windows end above the
  // visible dock instead of running behind it. Measured from the real dock so
  // theme-specific dock sizes and offsets are respected.
  const [dockReserve, setDockReserve] = useState(0);
  useEffect(() => {
    if (!clearance) { setDockReserve(0); return; }
    const dock = document.querySelector<HTMLElement>(".os-dock");
    if (!dock) { setDockReserve(0); return; }
    const measure = () => {
      const layerRect = layer.current?.getBoundingClientRect();
      const dockRect = dock.getBoundingClientRect();
      if (!layerRect || !dockRect.height) { setDockReserve(0); return; }
      setDockReserve(Math.max(0, Math.round(layerRect.bottom - dockRect.top)));
    };
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(dock);
    window.addEventListener("resize", measure);
    return () => { observer?.disconnect(); window.removeEventListener("resize", measure); };
  }, [clearance]);
  // `null` until the layer is measured; until then the server-rendered nonce
  // position style stays in effect (no jump from a default work area).
  const [area, setArea] = useState<WorkArea | null>(null);
  useEffect(() => {
    const element = layer.current;
    if (!element) return;
    const measure = () => { const rect = element.getBoundingClientRect(); if (rect.width && rect.height) setArea({ width: rect.width, height: rect.height }); };
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(element);
    window.addEventListener("resize", measure);
    return () => { observer?.disconnect(); window.removeEventListener("resize", measure); };
  }, []);
  // Focus history -> stacking: the focused window is on top (1), previously
  // focused windows step one level back (2, 3, …).
  const history = useRef<string[]>([]);
  const zIndex = useMemo(() => {
    const ids = state.windows.map((item) => item.id);
    const known = history.current.filter((id) => ids.includes(id));
    for (const id of ids) if (!known.includes(id)) known.unshift(id);
    const ordered = state.focusedWindowId && ids.includes(state.focusedWindowId)
      ? [state.focusedWindowId, ...known.filter((id) => id !== state.focusedWindowId)]
      : known;
    history.current = ordered;
    const map: { [id: string]: number } = {};
    ordered.forEach((id, index) => { map[id] = ordered.length - index; });
    for (const item of state.windows) if (item.fullscreen && !item.minimized) map[item.id] = 1000;
    return map;
  }, [state.windows, state.focusedWindowId]);
  return <div ref={layer} className="window-layer">
    <AnimatePresence initial={false}>{state.windows.map((item) =>
      <HostedWindow key={item.id} item={item} area={area} dockReserve={dockReserve} zIndex={zIndex[item.id] ?? 1} animations={animations} />
    )}</AnimatePresence>
  </div>;
}
function HostedWindow({ item, area, dockReserve, zIndex, animations: motionEnabled = true }: { item: ShellWindow; area: WorkArea | null; dockReserve: number; zIndex: number; animations?: boolean }) {
  const { state, dispatch, open, mode } = useShell();
  const { t } = useI18n();
  const workspace = useWorkspace();
  const { variants, tokens } = useTheme();
  const topbarInset = TOPBAR_HEIGHT * (Number.parseFloat(tokens["typography.scale"] ?? "1") || 1);
  const location = useLocation();
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const isLauncher = mode === "launcher" || mode === "mobile";
  const mobileShell = mode === "mobile";
  const [mobileNav, setMobileNav] = useState<MobileNavModel | null>(null);
  const registerMobileNav = useCallback((windowId: string, model: MobileNavModel | null) => {
    if (windowId === item.id) setMobileNav(model);
  }, [item.id]);
  const mobileNavSlot = useMemo(() => ({ windowId: item.id, register: registerMobileNav }), [item.id, registerMobileNav]);
  const flush = isInstalledAppWindow(item.id);
  // Chrome belongs to this window, not the globally active route. Otherwise
  // focusing Settings strips Files' header (and lends it to other windows).
  const topbarApp = item.id.startsWith("app:") ? findFirstPartyApp(item.id.slice(4)) : undefined;
  const customTopbar = topbarApp?.customTopbar === true;
  const customTopbarFullscreen = customTopbar && topbarApp?.customTopbarFullscreen === true;
  const isFullscreen = item.fullscreen === true;
  const hidden = item.minimized || (isLauncher && active !== item.id);
  // The usable desktop ends above the dock when clearance is on. Fullscreen
  // windows still own the whole screen; everything else respects the reserve.
  const workArea = area ? { width: area.width, height: Math.max(1, area.height - dockReserve) } : null;
  // With a reserved dock row, even floating windows keep their bottom edge
  // inside the usable desktop instead of hanging behind the dock.
  const constrain = (rect: WindowRect, target: WorkArea): WindowRect => {
    const next = constrainRect(rect, target);
    return dockReserve > 0 ? { ...next, y: Math.min(next.y, Math.max(0, target.height - next.height)) } : next;
  };
  const savedRect = readWindowRect(item.id);
  // Ignore the old generic frame and the Store's superseded 1040×720 default
  // so a wider preferred size is adopted without discarding user resizes.
  const supersededDefault = savedRect?.width === 1040 && savedRect.height === 720 && item.preferredSize && (item.preferredSize.width !== 1040 || item.preferredSize.height !== 720);
  const effectiveSaved = item.preferredSize && (supersededDefault || (savedRect?.width === 760 && savedRect.height === 540)) ? undefined : savedRect;
  const stored = item.rect ?? effectiveSaved ?? centeredRect(workArea ?? { width: 760, height: 540 }, item.preferredSize);
  const positionReady = useRef(false);
  useLayoutEffect(() => {
    if (!workArea) return;
    if (!positionReady.current) {
      positionReady.current = true;
      // The latest position takes precedence over the initial workspace preset.
      const initial = item.rect ?? effectiveSaved ?? centeredRect(workArea, item.preferredSize);
      if (initial !== item.rect) {
        dispatch({ type: "set-window-rect", id: item.id, rect: initial });
        return;
      }
    }
    if (item.rect) writeWindowRect(item.id, item.rect);
  }, [area, dockReserve, item.id, item.rect, dispatch]);
  const placement = item.placement ?? "floating";
  // Maximized desktop windows sit below the 54px top bar; only the launcher
  // fills the screen behind its own chrome. When the user hides the top bar,
  // the window takes that space instead of leaving an empty strip behind it.
  const chromeHidesTopbar = mode === "desktop" ? placement === "maximized" : mode === "launcher";
  const hideTopbar = (workspace.userWorkspace?.fullscreenChrome ?? workspace.effective.fullscreenChrome).hideTopbar;
  const windowTopInset = isFullscreen || (hideTopbar && chromeHidesTopbar)
    ? 0
    : topbarInset;
  const rect = area && workArea
    ? isFullscreen
      ? { x: 0, y: 0, width: area.width, height: area.height }
      : constrain(placedRect(stored, isLauncher ? "maximized" : placement, workArea, windowTopInset, isLauncher ? 0 : WINDOW_GAP), workArea)
    : stored;
  useEffect(() => {
    if (!isFullscreen) return;
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") dispatch({ type: "set-window-fullscreen", id: item.id, fullscreen: false });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, isFullscreen, item.id]);
  const gesture = useRef<null | { pointer: number; x: number; y: number; rect: typeof rect; resize: boolean; pending: null | { stored: typeof stored; ratio: number; offsetY: number }; element: HTMLElement | null; live: typeof rect; snap: WindowPlacement | null; moved: boolean }>(null);
  const [moving, setMoving] = useState(false);
  const [snap, setSnap] = useState<WindowPlacement | null>(null);
  function focus() {
    if (state.focusedWindowId !== item.id || item.minimized) dispatch({ type: "focus-window", id: item.id });
    if (active !== item.id && item.location) open(item.location);
  }
  function begin(event: PointerEvent<HTMLElement>, resize: boolean) {
    if (!workArea || isLauncher || event.button !== 0 || (!resize && (event.target as HTMLElement).closest("button, input, a, select"))) return;
    if (resize && placement !== "floating") return;
    event.preventDefault(); event.stopPropagation(); focus();
    const wrapper = event.currentTarget.closest(".window-position") as HTMLElement | null;
    wrapper?.classList.add("is-dragging");
    const element = (wrapper?.querySelector(".shell-window") as HTMLElement | null) ?? null;
    event.currentTarget.setPointerCapture(event.pointerId);
    if (!resize && placement !== "floating") {
      // Do NOT restore yet: only a real drag of a maximized window turns it
      // floating. A single click must do nothing; double-click toggles it.
      const title = event.currentTarget.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (event.clientX - title.left) / title.width));
      gesture.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, rect, resize: false, pending: { stored, ratio, offsetY: event.clientY - title.top }, element, live: rect, snap: null, moved: false };
      setMoving(true);
      return;
    }
    gesture.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, rect, resize, pending: null, element, live: rect, snap: null, moved: false };
    setMoving(true);
  }
  function move(event: PointerEvent) {
    const start = gesture.current;
    if (!start || start.pointer !== event.pointerId || !workArea) return;
    const dx = event.clientX - start.x, dy = event.clientY - start.y;
    if (start.pending) {
      if (Math.abs(dx) + Math.abs(dy) < 4) return;
      const under = start.pending;
      const next = constrain({ ...under.stored, x: event.clientX - under.stored.width * under.ratio, y: event.clientY - under.offsetY }, workArea);
      start.pending = null; start.rect = next; start.live = next; start.moved = true;
      start.x = event.clientX; start.y = event.clientY;
      dispatch({ type: "set-window-placement", id: item.id, placement: "floating" });
      dispatch({ type: "set-window-rect", id: item.id, rect: next });
      return;
    }
    if (start.resize) {
      const next = constrain({ ...start.rect, width: start.rect.width + dx, height: start.rect.height + dy }, workArea);
      start.live = next; start.moved = true;
      const element = start.element;
      if (element) { element.style.left = `${next.x}px`; element.style.top = `${next.y}px`; element.style.width = `${next.width}px`; element.style.height = `${next.height}px`; }
      return;
    }
    const next = constrain({ ...start.rect, x: start.rect.x + dx, y: start.rect.y + dy }, workArea);
    start.live = next; start.moved = true;
    // Move with a composited transform (no layout thrash); commit the rect on release.
    if (start.element) start.element.style.transform = `translate3d(${next.x - start.rect.x}px, ${next.y - start.rect.y}px, 0)`;
    const bounds = event.currentTarget.getBoundingClientRect();
    const target = snapTarget(event.clientX - bounds.left, event.clientY - bounds.top);
    if (target !== start.snap) { start.snap = target; setSnap(target); }
  }
  function snapTarget(x: number, y: number): WindowPlacement | null {
    if (!workArea) return null;
    return y <= 16 ? "maximized" : x <= 20 ? "left" : x >= workArea.width - 20 ? "right" : null;
  }
  function end() {
    const start = gesture.current;
    if (start?.moved) {
      const final = start.live;
      const element = start.element;
      if (element) {
        // Drop the transient transform and pin the committed position so the
        // state update does not jump the window.
        element.style.transform = "";
        element.style.left = `${final.x}px`;
        element.style.top = `${final.y}px`;
        element.style.width = `${final.width}px`;
        element.style.height = `${final.height}px`;
      }
      writeWindowRect(item.id, final);
      dispatch({ type: "set-window-rect", id: item.id, rect: final });
      if (!start.resize && start.snap) dispatch({ type: "set-window-placement", id: item.id, placement: start.snap });
    }
    // Flush the committed geometry while transitions are still disabled.
    // Otherwise the browser animates from the last transform back to zero.
    if (start?.element) {
      void start.element.offsetWidth;
      start.element.closest(".window-position")?.classList.remove("is-dragging");
    }
    gesture.current = null; setMoving(false); setSnap(null);
  }
  function keyboard(event: KeyboardEvent, resize: boolean) {
    if (!workArea || event.target !== event.currentTarget || isLauncher) return;
    if (event.altKey && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
      event.preventDefault();
      dispatch({ type: "set-window-placement", id: item.id, placement: event.key === "ArrowLeft" ? "left" : event.key === "ArrowRight" ? "right" : event.key === "ArrowUp" ? "maximized" : "floating" }); return;
    }
    if (placement !== "floating") return;
    const delta = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key];
    if (!delta) return;
    event.preventDefault(); const step = event.shiftKey ? 40 : 10;
    const next = resize ? { ...rect, width: rect.width + delta[0]! * step, height: rect.height + delta[1]! * step } : { ...rect, x: rect.x + delta[0]! * step, y: rect.y + delta[1]! * step };
    dispatch({ type: "set-window-rect", id: item.id, rect: constrain(next, workArea) });
  }
  const position = useRef<HTMLDivElement>(null);
  // Write the focus z-index through CSSOM as well as the style prop. React's
  // initial style string is not always parsed for numeric z-index in a freshly
  // hydrated tree, which left a newly focused window behind the old one.
  useLayoutEffect(() => { if (position.current) position.current.style.zIndex = String(zIndex); }, [zIndex]);
  const animate = motionEnabled && typeof window !== "undefined" && typeof window.matchMedia === "function" && !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const enter = isLauncher ? { opacity: 0, y: 140, scale: 0.94 } : { opacity: 0, y: 10, scale: 0.985 };
  // The focus z-index lives on a plain wrapper: motion owns the inner element's
  // style for its entrance animation and drops foreign style props.
  return <div
    aria-hidden={hidden ? true : undefined}
    ref={position}
    className={`window-position${moving ? " is-moving is-dragging" : ""}${hidden ? " is-hidden" : ""}`}
    data-fullscreen={isFullscreen ? "true" : undefined}
    style={{ zIndex }}>
  <motion.div
    className="window-motion"
    {...(animate ? { initial: enter, exit: enter } : { initial: false })}
    animate={{ opacity: 1, y: 0, scale: 1 }}
    transition={isLauncher ? { type: "spring", stiffness: 320, damping: 30 } : { type: "spring", stiffness: 430, damping: 34, mass: 0.82 }}
    onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end}>
    {snap ? <div aria-hidden="true" className={`snap-preview snap-preview--${snap}`} /> : null}
    <MobileNavSlotContext.Provider value={mobileShell ? mobileNavSlot : null}>
    <ProtectedWindow customTopbar={customTopbar} fullscreenTopbar={customTopbarFullscreen} flush={flush} id={item.id} focused={state.focusedWindowId === item.id}
      style={area ? { left: rect.x, top: rect.y, width: rect.width, height: rect.height } : undefined}
      onClose={() => { dispatch({ type: "close-window", id: item.id }); if (active === item.id) open("/"); }}
      onFocus={focus} onMinimize={() => { dispatch({ type: "toggle-minimize", id: item.id }); if (active === item.id) open("/"); }}
      onMaximize={isLauncher ? undefined : () => dispatch({ type: "set-window-placement", id: item.id, placement: placement !== "floating" ? "floating" : "maximized" })}
      onFullscreen={isLauncher ? undefined : () => dispatch({ type: "set-window-fullscreen", id: item.id, fullscreen: !isFullscreen })}
      onSnap={isLauncher ? undefined : (side) => dispatch({ type: "set-window-placement", id: item.id, placement: side })}
      fullscreen={isFullscreen} frameless={isFullscreen && !customTopbarFullscreen}
      maximized={placement !== "floating" || isFullscreen} {...(!isLauncher ? { onTitlePointerDown: (event: PointerEvent<HTMLElement>) => begin(event, false), onTitleKeyDown: (event: KeyboardEvent<HTMLElement>) => keyboard(event, false) } : {})}
      onResizePointerDown={(event) => begin(event, true)} onResizeKeyDown={(event) => keyboard(event, true)}
      subtitle={item.subtitle} title={item.title} stream={item.streamId !== undefined} variant={variants.windowChrome}>
      <RouteBoundary location={item.location ?? "/"}><ShellRoutes location={item.location ?? "/"} /></RouteBoundary>
    </ProtectedWindow>
    </MobileNavSlotContext.Provider>
    {isFullscreen ? <button type="button" className="window-exit-fullscreen" onClick={() => dispatch({ type: "set-window-fullscreen", id: item.id, fullscreen: false })} aria-label={t("window.exitFullscreen")}>
      <WindowExitFullscreenIcon aria-hidden="true" /><span>{t("window.exitFullscreen")}</span>
    </button> : null}
    {mobileShell ? <MobileAppNav
      model={mobileNav}
      onSwitcher={() => dispatch({ type: "toggle-app-switcher" })}
    /> : null}
  </motion.div>
  </div>;
}
