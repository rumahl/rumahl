import { useEffect, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

export interface AnchoredPopoverProps {
  anchor: RefObject<HTMLElement | null>;
  open: boolean;
  align?: "start" | "end";
  /** Gap below the anchor in px. */
  offset?: number;
  className?: string;
  children: ReactNode;
}

/**
 * Portals a popover to `document.body` and pins it under its anchor. Living
 * outside a backdrop-filtered container lets the popover's own blur work.
 */
export function AnchoredPopover({
  anchor,
  open,
  align = "start",
  offset = 5,
  className = "",
  children
}: AnchoredPopoverProps) {
  const [position, setPosition] = useState<{ top: number; left?: number; right?: number } | null>(null);
  useEffect(() => {
    if (!open) return;
    const place = () => {
      const element = anchor.current;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      setPosition(
        align === "end"
          ? { top: rect.bottom + offset, right: Math.max(8, window.innerWidth - rect.right) }
          : { top: rect.bottom + offset, left: rect.left }
      );
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, align, anchor, offset]);
  if (!open || !position || typeof document === "undefined") return null;
  return createPortal(
    <div
      className={`rumahl-popover ${className}`}
      role="menu"
      style={{ position: "fixed", top: position.top, left: position.left, right: position.right, zIndex: 120 }}
    >
      {children}
    </div>,
    document.body
  );
}
