import { useEffect, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

/**
 * Portals a menu dropdown to `document.body` and positions it under its anchor.
 * Living outside the (backdrop-filtered) top bar lets its own backdrop filter
 * blur the page content behind it.
 */
export function MenuDropdown({ anchor, open, align = "left", className, children }: {
  anchor: RefObject<HTMLElement | null>;
  open: boolean;
  align?: "left" | "right";
  className?: string;
  children: ReactNode;
}) {
  const [position, setPosition] = useState<{ top: number; left?: number; right?: number } | null>(null);
  useEffect(() => {
    if (!open) return;
    const place = () => {
      const element = anchor.current;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      setPosition(align === "right"
        ? { top: rect.bottom + 5, right: Math.max(8, window.innerWidth - rect.right) }
        : { top: rect.bottom + 5, left: rect.left });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [open, align, anchor]);
  if (!open || !position || typeof document === "undefined") return null;
  return createPortal(
    <div className={className} role="menu" style={{ position: "fixed", top: position.top, left: position.left, right: position.right, zIndex: 120 }}>{children}</div>,
    document.body
  );
}
