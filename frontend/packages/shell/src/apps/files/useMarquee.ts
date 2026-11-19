import { useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { rectsIntersect, type SelectionRect } from "../../shell/desktop/geometry";

interface ItemRect extends SelectionRect {
  key: string;
}

interface MarqueeStart {
  x: number;
  y: number;
  base: Set<string>;
  box: DOMRect;
  scrollLeft: number;
  scrollTop: number;
  rects: ItemRect[];
}

/**
 * Desktop-style marquee (rubber-band) selection for a scroll container. Items
 * opt in with `data-file-key`; the rect is drawn relative to the scrolled
 * content and intersects item boxes to build the selection.
 *
 * Item boxes are measured once at pointer-down and the pointer is sampled at
 * most once per animation frame. A drag therefore does not force a layout read
 * per item per pointer move.
 */
export function useMarquee(
  surface: RefObject<HTMLElement | null>,
  selected: ReadonlySet<string>,
  setSelection: (keys: Iterable<string>) => void,
) {
  const start = useRef<MarqueeStart | null>(null);
  const pending = useRef<{ x: number; y: number } | null>(null);
  const frame = useRef<number | null>(null);
  const [rect, setRect] = useState<SelectionRect | null>(null);

  function cancelFrame() {
    if (frame.current !== null && typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame.current);
    frame.current = null;
    pending.current = null;
  }

  function onPointerDown(event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("[data-file-key], button, input, a, label, .files-menu")) return;
    const host = surface.current;
    if (!host) return;
    const additive = event.ctrlKey || event.metaKey || event.shiftKey;
    const box = host.getBoundingClientRect();
    const rects: ItemRect[] = [];
    host.querySelectorAll<HTMLElement>("[data-file-key]").forEach((element) => {
      const key = element.dataset.fileKey;
      if (!key) return;
      const item = element.getBoundingClientRect();
      rects.push({
        key,
        x: item.left - box.left + host.scrollLeft,
        y: item.top - box.top + host.scrollTop,
        width: item.width,
        height: item.height,
      });
    });
    cancelFrame();
    start.current = {
      x: event.clientX,
      y: event.clientY,
      base: additive ? new Set(selected) : new Set(),
      box,
      scrollLeft: host.scrollLeft,
      scrollTop: host.scrollTop,
      rects,
    };
    if (!additive) setSelection([]);
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function measure(x: number, y: number) {
    const begin = start.current;
    if (!begin) return;
    const width = Math.abs(x - begin.x);
    const height = Math.abs(y - begin.y);
    if (width < 6 && height < 6) return;
    const area: SelectionRect = {
      x: Math.min(begin.x, x) - begin.box.left + begin.scrollLeft,
      y: Math.min(begin.y, y) - begin.box.top + begin.scrollTop,
      width,
      height,
    };
    setRect(area);
    const next = new Set(begin.base);
    for (const item of begin.rects) if (rectsIntersect(area, item)) next.add(item.key);
    setSelection(next);
  }

  function onPointerMove(event: ReactPointerEvent<HTMLElement>) {
    if (!start.current || !surface.current) return;
    pending.current = { x: event.clientX, y: event.clientY };
    if (frame.current !== null) return;
    if (typeof requestAnimationFrame !== "function") {
      const point = pending.current;
      pending.current = null;
      if (point) measure(point.x, point.y);
      return;
    }
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      const point = pending.current;
      pending.current = null;
      if (point) measure(point.x, point.y);
    });
  }

  function onPointerUp() {
    cancelFrame();
    start.current = null;
    setRect(null);
  }

  return { rect, handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel: onPointerUp } };
}
