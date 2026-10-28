import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";

export interface MobileIslandNavItem {
  id: string;
  label: string;
  icon: ReactNode;
}

export interface MobileIslandNavProps {
  items: readonly MobileIslandNavItem[];
  value: string;
  onChange: (id: string) => void;
  /** Accessible name for the navigation landmark. */
  label: string;
  className?: string;
}

interface IndicatorMetrics {
  left: number;
  width: number;
  top: number;
  height: number;
}

interface DragState {
  pointerId: number;
  startX: number;
  moved: boolean;
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/**
 * Floating bottom navigation for narrow app layouts. A tap changes selection
 * through the native click path, so the active pill animates into place. Once
 * the pointer moves horizontally, the pill becomes draggable and tracks it;
 * selection commits on release. Keyboard navigation uses the same model.
 */
export function MobileIslandNav({ items, value, onChange, label, className = "" }: MobileIslandNavProps) {
  const navRef = useRef<HTMLElement>(null);
  const itemsRef = useRef(new Map<string, HTMLButtonElement>());
  const dragRef = useRef<DragState | null>(null);
  const previewRef = useRef<string | null>(null);
  const suppressClick = useRef(false);
  const [metrics, setMetrics] = useState<IndicatorMetrics>({ left: 0, width: 0, top: 0, height: 0 });
  const [dragging, setDragging] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [dragLeft, setDragLeft] = useState<number | null>(null);
  const active = preview ?? value;

  const measure = useCallback(() => {
    const item = itemsRef.current.get(active);
    if (!item) return;
    setMetrics({ left: item.offsetLeft, width: item.offsetWidth, top: item.offsetTop, height: item.offsetHeight });
  }, [active]);

  useLayoutEffect(measure, [measure, items.length]);
  useEffect(() => {
    const nav = navRef.current;
    if (!nav) return;
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(nav);
    itemsRef.current.forEach((item) => observer?.observe(item));
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [measure, items.length]);

  const itemAt = (clientX: number): { id: string; element: HTMLButtonElement } | null => {
    const nav = navRef.current;
    if (!nav) return null;
    const x = clientX - nav.getBoundingClientRect().left;
    let best: { id: string; element: HTMLButtonElement; distance: number } | null = null;
    for (const [id, element] of itemsRef.current) {
      const center = element.offsetLeft + element.offsetWidth / 2;
      const distance = Math.abs(x - center);
      if (!best || distance < best.distance) best = { id, element, distance };
    }
    return best;
  };

  const indicatorLeft = (clientX: number, width: number): number => {
    const elements = [...itemsRef.current.values()];
    if (elements.length === 0) return 0;
    const first = elements[0]!;
    const last = elements[elements.length - 1]!;
    const min = first.offsetLeft;
    const max = last.offsetLeft + last.offsetWidth - width;
    return clamp(clientX - (navRef.current?.getBoundingClientRect().left ?? 0) - width / 2, min, max);
  };

  const resetDrag = () => {
    dragRef.current = null;
    previewRef.current = null;
    setDragging(false);
    setPreview(null);
    setDragLeft(null);
  };

  const startDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if (event.isPrimary === false) return;
    suppressClick.current = false;
    dragRef.current = { pointerId: event.pointerId, startX: event.clientX, moved: false };
  };

  const moveDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const current = dragRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    if (!current.moved) {
      if (Math.abs(event.clientX - current.startX) < 5) return;
      current.moved = true;
      event.currentTarget.setPointerCapture?.(event.pointerId);
      setDragging(true);
    }
    const picked = itemAt(event.clientX);
    if (!picked) return;
    previewRef.current = picked.id;
    setPreview(picked.id);
    setDragLeft(indicatorLeft(event.clientX, picked.element.offsetWidth));
  };

  const finishDrag = (event: ReactPointerEvent<HTMLElement>, commit: boolean) => {
    const current = dragRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const next = current.moved ? previewRef.current : null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture?.(event.pointerId);
    }
    resetDrag();
    if (!next || !commit) return;
    suppressClick.current = true;
    window.setTimeout(() => { suppressClick.current = false; }, 50);
    const item = itemsRef.current.get(next);
    item?.focus({ preventScroll: true });
    onChange(next);
  };

  const handleClick = (id: string) => {
    // A tap has not started a drag; let the native click drive the animated
    // selection change. Drags suppress this click in the capture phase.
    onChange(id);
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    const current = items.findIndex((item) => item.id === value);
    const delta = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : current + delta;
    if ((delta === 0 && event.key !== "Home" && event.key !== "End") || next < 0 || next >= items.length) return;
    event.preventDefault();
    const item = items[next]!;
    onChange(item.id);
    requestAnimationFrame(() => itemsRef.current.get(item.id)?.focus({ preventScroll: true }));
  };

  const indicatorStyle = {
    left: dragLeft ?? metrics.left,
    width: metrics.width,
    top: metrics.top,
    height: metrics.height
  } as CSSProperties;

  return <nav
    ref={navRef}
    aria-label={label}
    className={`rumahl-island${dragging ? " is-dragging" : ""}${className ? ` ${className}` : ""}`}
    data-dragging={dragging ? "true" : undefined}
    onPointerDown={startDrag}
    onPointerMove={moveDrag}
    onPointerUp={(event) => finishDrag(event, true)}
    onPointerCancel={(event) => finishDrag(event, false)}
    onLostPointerCapture={() => resetDrag()}
    onKeyDown={handleKeyDown}
    onClickCapture={(event) => {
      if (suppressClick.current && event.detail > 0) {
        event.preventDefault();
        event.stopPropagation();
      }
    }}
  >
    <span className="rumahl-island__indicator" aria-hidden="true" style={indicatorStyle} />
    {items.map((item) => <button
      key={item.id}
      ref={(element) => {
        if (element) itemsRef.current.set(item.id, element);
        else itemsRef.current.delete(item.id);
      }}
      type="button"
      className="rumahl-island__item"
      aria-current={active === item.id ? "page" : undefined}
      data-drag-preview={preview === item.id && dragging ? "true" : undefined}
      onClick={() => handleClick(item.id)}
    >
      <span className="rumahl-island__icon" aria-hidden="true">{item.icon}</span>
      <span>{item.label}</span>
    </button>)}
  </nav>;
}
