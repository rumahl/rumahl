import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useLocation } from "react-router";
import { useTheme } from "@rumahl/ui";
import { ProtectedWindow } from "../components/ProtectedWindow";
import { findFirstPartyApp } from "../apps/registry";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { RouteBoundary } from "../routing/RouteBoundary";
import { useShell } from "./ShellContext";
import type { ShellWindow } from "../shell-state";
import { centeredRect, constrainRect, placedRect, type WindowPlacement, type WorkArea } from "./desktop/geometry";

import { readWindowRect, writeWindowRect } from "./window-positions";

/** Height of the flush top bar maximized/snapped windows sit below. */
const TOPBAR_HEIGHT = 38;

/**
 * True for an installed (sandboxed iframe) app window. First-party React apps,
 * streams and pages are not flush.
 */
export function isInstalledAppWindow(id: string): boolean {
  const appId = id.startsWith("app:") ? id.slice("app:".length) : undefined;
  return appId !== undefined && findFirstPartyApp(appId) === undefined;
}

export function DesktopWindows() {
  const { state } = useShell();
  const layer = useRef<HTMLDivElement>(null);
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
    return map;
  }, [state.windows, state.focusedWindowId]);
  return <div ref={layer} className="window-layer">
    <AnimatePresence initial={false}>{state.windows.map((item) =>
      <HostedWindow key={item.id} item={item} area={area} zIndex={zIndex[item.id] ?? 1} />
    )}</AnimatePresence>
  </div>;
}
function HostedWindow({ item, area, zIndex }: { item: ShellWindow; area: WorkArea | null; zIndex: number }) {
  const { state, dispatch, open, mode } = useShell();
  const { variants } = useTheme();
  const location = useLocation();
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const isLauncher = mode === "launcher";
  const flush = isInstalledAppWindow(item.id);
  const hidden = item.minimized || (isLauncher && active !== item.id);
  const stored = item.rect ?? centeredRect(area ?? { width: 760, height: 540 });
  const positionReady = useRef(false);
  useLayoutEffect(() => {
    if (!area) return;
    if (!positionReady.current) {
      positionReady.current = true;
      // The latest position takes precedence over the initial workspace preset.
      const initial = readWindowRect(item.id) ?? item.rect ?? centeredRect(area);
      if (initial !== item.rect) {
        dispatch({ type: "set-window-rect", id: item.id, rect: initial });
        return;
      }
    }
    if (item.rect) writeWindowRect(item.id, item.rect);
  }, [area, item.id, item.rect, dispatch]);
  const placement = item.placement ?? "floating";
  const rect = area ? placedRect(stored, isLauncher ? "maximized" : placement, area, TOPBAR_HEIGHT) : stored;
  const gesture = useRef<null | { pointer: number; x: number; y: number; rect: typeof rect; resize: boolean; pending: null | { stored: typeof stored; ratio: number; offsetY: number }; element: HTMLElement | null; live: typeof rect; snap: WindowPlacement | null; moved: boolean }>(null);
  const [moving, setMoving] = useState(false);
  const [snap, setSnap] = useState<WindowPlacement | null>(null);
  function focus() {
    if (state.focusedWindowId !== item.id || item.minimized) dispatch({ type: "focus-window", id: item.id });
    if (active !== item.id && item.location) open(item.location);
  }
  function begin(event: PointerEvent<HTMLElement>, resize: boolean) {
    if (!area || isLauncher || event.button !== 0 || (!resize && (event.target as HTMLElement).closest("button, input, a, select"))) return;
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
    if (!start || start.pointer !== event.pointerId || !area) return;
    const dx = event.clientX - start.x, dy = event.clientY - start.y;
    if (start.pending) {
      if (Math.abs(dx) + Math.abs(dy) < 4) return;
      const under = start.pending;
      const next = constrainRect({ ...under.stored, x: event.clientX - under.stored.width * under.ratio, y: event.clientY - under.offsetY }, area);
      start.pending = null; start.rect = next; start.live = next; start.moved = true;
      start.x = event.clientX; start.y = event.clientY;
      dispatch({ type: "set-window-placement", id: item.id, placement: "floating" });
      dispatch({ type: "set-window-rect", id: item.id, rect: next });
      return;
    }
    if (start.resize) {
      const next = constrainRect({ ...start.rect, width: start.rect.width + dx, height: start.rect.height + dy }, area);
      start.live = next; start.moved = true;
      const element = start.element;
      if (element) { element.style.left = `${next.x}px`; element.style.top = `${next.y}px`; element.style.width = `${next.width}px`; element.style.height = `${next.height}px`; }
      return;
    }
    const next = constrainRect({ ...start.rect, x: start.rect.x + dx, y: start.rect.y + dy }, area);
    start.live = next; start.moved = true;
    // Move with a composited transform (no layout thrash); commit the rect on release.
    if (start.element) start.element.style.transform = `translate3d(${next.x - start.rect.x}px, ${next.y - start.rect.y}px, 0)`;
    const bounds = event.currentTarget.getBoundingClientRect();
    const target = snapTarget(event.clientX - bounds.left, event.clientY - bounds.top);
    if (target !== start.snap) { start.snap = target; setSnap(target); }
  }
  function snapTarget(x: number, y: number): WindowPlacement | null {
    if (!area) return null;
    return y <= 16 ? "maximized" : x <= 20 ? "left" : x >= area.width - 20 ? "right" : null;
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
    if (!area || event.target !== event.currentTarget || isLauncher) return;
    if (event.altKey && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
      event.preventDefault();
      dispatch({ type: "set-window-placement", id: item.id, placement: event.key === "ArrowLeft" ? "left" : event.key === "ArrowRight" ? "right" : event.key === "ArrowUp" ? "maximized" : "floating" }); return;
    }
    if (placement !== "floating") return;
    const delta = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key];
    if (!delta) return;
    event.preventDefault(); const step = event.shiftKey ? 40 : 10;
    const next = resize ? { ...rect, width: rect.width + delta[0]! * step, height: rect.height + delta[1]! * step } : { ...rect, x: rect.x + delta[0]! * step, y: rect.y + delta[1]! * step };
    dispatch({ type: "set-window-rect", id: item.id, rect: constrainRect(next, area) });
  }
  const animate = typeof window !== "undefined" && typeof window.matchMedia === "function" && !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const enter = isLauncher ? { opacity: 0, y: 140 } : { opacity: 0 };
  return <motion.div
    aria-hidden={hidden ? true : undefined}
    className={`window-position${moving ? " is-moving is-dragging" : ""}${hidden ? " is-hidden" : ""}`}
    style={{ zIndex }}
    {...(animate ? { initial: enter, exit: enter } : { initial: false })}
    animate={{ opacity: 1, y: 0 }}
    transition={isLauncher ? { type: "spring", stiffness: 320, damping: 30 } : { duration: 0.22, ease: [0.2, 0.7, 0.2, 1] }}
    onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end}>
    {snap ? <div aria-hidden="true" className={`snap-preview snap-preview--${snap}`} /> : null}
    <ProtectedWindow frameless={isLauncher} flush={flush} id={item.id} focused={state.focusedWindowId === item.id}
      style={area ? { left: rect.x, top: rect.y, width: rect.width, height: rect.height } : undefined}
      onClose={() => { dispatch({ type: "close-window", id: item.id }); if (active === item.id) open("/"); }}
      onFocus={focus} onMinimize={() => { dispatch({ type: "toggle-minimize", id: item.id }); if (active === item.id) open("/"); }}
      onMaximize={isLauncher ? undefined : () => dispatch({ type: "set-window-placement", id: item.id, placement: placement !== "floating" ? "floating" : "maximized" })}
      onSnap={isLauncher ? undefined : (side) => dispatch({ type: "set-window-placement", id: item.id, placement: side })}
      maximized={placement !== "floating"} onTitlePointerDown={(event) => begin(event, false)} onTitleKeyDown={(event) => keyboard(event, false)}
      onResizePointerDown={(event) => begin(event, true)} onResizeKeyDown={(event) => keyboard(event, true)}
      subtitle={item.subtitle} title={item.title} stream={item.streamId !== undefined} variant={variants.windowChrome}>
      <RouteBoundary location={item.location ?? "/"}><ShellRoutes location={item.location ?? "/"} /></RouteBoundary>
    </ProtectedWindow>
  </motion.div>;
}
