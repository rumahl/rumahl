import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useLocation } from "react-router";
import { useTheme } from "@rumahl/ui";
import { ProtectedWindow } from "../components/ProtectedWindow";
import { describeRoute, ShellRoutes } from "../routing/routes";
import { RouteBoundary } from "../routing/RouteBoundary";
import { useShell } from "./ShellContext";
import type { ShellWindow } from "../shell-state";
import { constrainRect, placedRect, type WindowPlacement, type WorkArea } from "./desktop/geometry";

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
  return <div ref={layer} className="window-layer">{state.windows.map((item, index) =>
    <HostedWindow key={item.id} item={item} area={area} order={index} />
  )}</div>;
}
function HostedWindow({ item, area, order }: { item: ShellWindow; area: WorkArea | null; order: number }) {
  const { state, dispatch, open, mode } = useShell();
  const { variants } = useTheme();
  const location = useLocation();
  const active = describeRoute(location.pathname).id;
  const isLauncher = mode === "launcher";
  const hidden = item.minimized || (isLauncher && active !== item.id);
  const stored = item.rect ?? { x: 36, y: 24, width: 760, height: 540 };
  const placement = item.placement ?? "floating";
  const rect = area ? placedRect(stored, isLauncher ? "maximized" : placement, area) : stored;
  const gesture = useRef<null | { pointer: number; x: number; y: number; rect: typeof rect; resize: boolean }>(null);
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
    let startRect = rect;
    if (placement !== "floating") {
      const title = event.currentTarget.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (event.clientX - title.left) / title.width));
      startRect = constrainRect({ ...stored, x: rect.x + event.clientX - title.left - stored.width * ratio, y: rect.y }, area);
      dispatch({ type: "set-window-placement", id: item.id, placement: "floating" });
      dispatch({ type: "set-window-rect", id: item.id, rect: startRect });
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    gesture.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, rect: startRect, resize };
    setMoving(true);
  }
  function move(event: PointerEvent) {
    const start = gesture.current;
    if (!start || start.pointer !== event.pointerId || !area) return;
    const dx = event.clientX - start.x, dy = event.clientY - start.y;
    if (!start.resize) {
      const bounds = event.currentTarget.getBoundingClientRect();
      setSnap(snapTarget(event.clientX - bounds.left, event.clientY - bounds.top));
    }
    const next = start.resize ? { ...start.rect, width: start.rect.width + dx, height: start.rect.height + dy } : { ...start.rect, x: start.rect.x + dx, y: start.rect.y + dy };
    dispatch({ type: "set-window-rect", id: item.id, rect: constrainRect(next, area) });
  }
  function snapTarget(x: number, y: number): WindowPlacement | null {
    if (!area) return null;
    return y <= 16 ? "maximized" : x <= 20 ? "left" : x >= area.width - 20 ? "right" : null;
  }
  function end(event?: PointerEvent) {
    if (event?.type === "pointerup" && gesture.current && !gesture.current.resize) {
      const bounds = event.currentTarget.getBoundingClientRect();
      const x = event.clientX - bounds.left, y = event.clientY - bounds.top;
      const placement = snapTarget(x, y);
      if (placement) dispatch({ type: "set-window-placement", id: item.id, placement });
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
  return <div hidden={hidden} className={`window-position${moving ? " is-moving" : ""}`} ref={(element) => { if (element) element.style.zIndex = String(order + 1); }}
    onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end}>
    {snap ? <div aria-hidden="true" className={`snap-preview snap-preview--${snap}`} /> : null}
    <ProtectedWindow frameless={isLauncher} id={item.id} focused={state.focusedWindowId === item.id}
      style={area ? { left: rect.x, top: rect.y, width: rect.width, height: rect.height } : undefined}
      onClose={() => { dispatch({ type: "close-window", id: item.id }); if (active === item.id) open("/"); }}
      onFocus={focus} onMinimize={() => { dispatch({ type: "toggle-minimize", id: item.id }); if (active === item.id) open("/"); }}
      onMaximize={() => dispatch({ type: "set-window-placement", id: item.id, placement: placement !== "floating" ? "floating" : "maximized" })}
      onSnap={(side) => dispatch({ type: "set-window-placement", id: item.id, placement: side })}
      maximized={placement !== "floating"} onTitlePointerDown={(event) => begin(event, false)} onTitleKeyDown={(event) => keyboard(event, false)}
      onResizePointerDown={(event) => begin(event, true)} onResizeKeyDown={(event) => keyboard(event, true)}
      subtitle={item.subtitle} title={item.title} stream={item.streamId !== undefined} variant={variants.windowChrome}>
      <RouteBoundary location={item.location ?? "/"}><ShellRoutes location={item.location ?? "/"} /></RouteBoundary>
    </ProtectedWindow>
  </div>;
}
