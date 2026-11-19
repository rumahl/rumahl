import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

export interface LauncherMenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

/** A small context menu for launcher items (apps, folders, empty space). */
export function LauncherMenu({ x, y, items, onClose }: { x: number; y: number; items: LauncherMenuItem[]; onClose: () => void }) {
  useEffect(() => {
    const close = () => onClose();
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);
  if (typeof document === "undefined") return null;
  const left = Math.min(x, Math.max(8, window.innerWidth - 240));
  const top = Math.min(y, Math.max(8, window.innerHeight - 300));
  return createPortal(
    <div className="desktop-menu" role="menu"
      ref={(element) => { if (element) { element.style.left = `${left}px`; element.style.top = `${top}px`; } }}
      onPointerDown={(event) => event.stopPropagation()}>
      {items.map((item) => <button key={item.label} role="menuitem" type="button" className={item.danger ? "is-danger" : undefined} disabled={item.disabled}
        onClick={() => { item.onSelect(); onClose(); }}>{item.icon}{item.label}</button>)}
    </div>,
    document.body
  );
}
