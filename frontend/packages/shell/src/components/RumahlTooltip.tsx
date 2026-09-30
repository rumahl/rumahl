import { cloneElement, useEffect, useId, useRef, useState, type ReactElement } from "react";
import { useControlPopover } from "./useControlPopover";

/** Hover and keyboard tooltip; the top layer avoids clipping inside windows. */
export function RumahlTooltip({ content, children }: { content: string; children: ReactElement<{ "aria-describedby"?: string | undefined }> }) {
  const [open, setOpen] = useState(false);
  const { triggerRef, popupRef } = useControlPopover<HTMLSpanElement>(open, setOpen);
  const id = useId();
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clear = () => { if (timer.current) clearTimeout(timer.current); };
  const hide = () => { clear(); setOpen(false); };
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { clear(); setOpen(false); } };
    window.addEventListener("keydown", escape);
    return () => { clear(); window.removeEventListener("keydown", escape); };
  }, []);
  return <span className="rumahl-tooltip-anchor" ref={triggerRef}
    onPointerEnter={event => { if (event.pointerType !== "touch") { clear(); timer.current = setTimeout(() => setOpen(true), 400); } }}
    onPointerLeave={hide} onFocus={() => { clear(); setOpen(true); }}
    onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget)) hide(); }}>
    {cloneElement(children, { "aria-describedby": [children.props["aria-describedby"], open ? id : undefined].filter(Boolean).join(" ") || undefined })}
    {open ? <div ref={popupRef} id={id} role="tooltip" popover="manual" className="rumahl-tooltip">{content}</div> : null}
  </span>;
}
