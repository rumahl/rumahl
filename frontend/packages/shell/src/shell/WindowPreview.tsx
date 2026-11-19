import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cloneWindowPreview } from "./window-preview";

export function WindowPreview({ windowId, fallback }: { windowId: string; fallback: ReactNode }) {
  const host = useRef<HTMLDivElement>(null);
  const [available, setAvailable] = useState(false);
  useLayoutEffect(() => {
    const target = host.current;
    const source = [...document.querySelectorAll<HTMLElement>(".window-layer [data-window-id]")]
      .find(element => element.dataset.windowId === windowId)?.querySelector<HTMLElement>(".shell-window__content");
    if (!target || !source) return;
    const copy = cloneWindowPreview(source);
    if (!copy) return;
    const width = source.clientWidth || 400;
    const height = source.clientHeight || 640;
    copy.style.width = `${width}px`;
    copy.style.height = `${height}px`;
    copy.style.overflow = "hidden";
    copy.style.pointerEvents = "none";
    target.append(copy);
    const resize = () => { copy.style.transform = `scale(${target.clientWidth / width})`; };
    resize();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(resize);
    observer?.observe(target);
    setAvailable(true);
    return () => { observer?.disconnect(); copy.remove(); };
  }, [windowId]);
  return <div className="app-switcher__preview" aria-hidden="true" inert>
    <div ref={host} className="app-switcher__snapshot" />
    {!available ? <div className="app-switcher__fallback">{fallback}</div> : null}
  </div>;
}
