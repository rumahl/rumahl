import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  onSelect: () => void;
}

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  useEffect(() => {
    const close = () => onClose();
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    return () => { window.removeEventListener("pointerdown", close); window.removeEventListener("blur", close); };
  }, [onClose]);
  const menu = <div className="files-menu" role="menu" style={{ left: x, top: y }} onPointerDown={(event) => event.stopPropagation()}>
    {items.map((item) => <button key={item.label} type="button" role="menuitem" className={item.danger ? "is-danger" : undefined} onClick={() => { item.onSelect(); onClose(); }}>
      {item.icon}{item.label}
    </button>)}
  </div>;
  return typeof document === "undefined" ? menu : createPortal(menu, document.body);
}
