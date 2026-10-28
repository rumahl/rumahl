import { useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { rectsIntersect, type SelectionRect } from "../../shell/desktop/geometry";

/**
 * Desktop-style marquee (rubber-band) selection for a scroll container. Items
 * opt in with `data-file-key`; the rect is drawn relative to the scrolled
 * content and intersects item boxes to build the selection.
 */
export function useMarquee(
  surface: RefObject<HTMLElement | null>,
  selected: ReadonlySet<string>,
  setSelection: (keys: Iterable<string>) => void,
) {
  const start = useRef<{ x: number; y: number; base: Set<string> } | null>(null);
  const [rect, setRect] = useState<SelectionRect | null>(null);

  function onPointerDown(event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("[data-file-key], button, input, a, label, .files-menu")) return;
    const additive = event.ctrlKey || event.metaKey || event.shiftKey;
    start.current = { x: event.clientX, y: event.clientY, base: additive ? new Set(selected) : new Set() };
    if (!additive) setSelection([]);
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function onPointerMove(event: ReactPointerEvent<HTMLElement>) {
    const begin = start.current;
    const host = surface.current;
    if (!begin || !host) return;
    const box = host.getBoundingClientRect();
    const width = Math.abs(event.clientX - begin.x);
    const height = Math.abs(event.clientY - begin.y);
    if (width < 6 && height < 6) return;
    const offsetX = Math.min(begin.x, event.clientX) - box.left + host.scrollLeft;
    const offsetY = Math.min(begin.y, event.clientY) - box.top + host.scrollTop;
    const area: SelectionRect = { x: offsetX, y: offsetY, width, height };
    setRect(area);
    const next = new Set(begin.base);
    host.querySelectorAll<HTMLElement>("[data-file-key]").forEach((element) => {
      const key = element.dataset.fileKey;
      if (!key) return;
      const item = element.getBoundingClientRect();
      const relative: SelectionRect = {
        x: item.left - box.left + host.scrollLeft,
        y: item.top - box.top + host.scrollTop,
        width: item.width,
        height: item.height,
      };
      if (rectsIntersect(area, relative)) next.add(key);
    });
    setSelection(next);
  }

  function onPointerUp() {
    start.current = null;
    setRect(null);
  }

  return { rect, handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel: onPointerUp } };
}
