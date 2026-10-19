import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { MenuDivider, MenuIconContext, MenuItem } from "../Menu/Menu";

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
}

export type ContextMenuEntry = ContextMenuItem | "separator";

export interface ContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  items: readonly ContextMenuEntry[];
  onClose: () => void;
  label?: string;
}

/**
 * Right-click (or long-press) context menu. Renders a `Menu` at the pointer,
 * clamped to the viewport, and closes on outside click, Escape, scroll or
 * resize. Pair with {@link useContextMenu}.
 */
export function ContextMenu({ open, x, y, items, onClose, label = "Context menu" }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) onClose();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    window.addEventListener("scroll", onClose, true);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("scroll", onClose, true);
    };
  }, [open, onClose]);

  if (!open || typeof document === "undefined") return null;
  const left = Math.max(8, Math.min(x, window.innerWidth - 220));
  const top = Math.max(8, Math.min(y, window.innerHeight - 8));
  const hasIcon = items.some((entry) => entry !== "separator" && entry.icon != null);

  return createPortal(
    <div
      ref={ref}
      className="rumahl-menu rumahl-context-menu"
      role="menu"
      aria-label={label}
      style={{ position: "fixed", left, top, zIndex: 220 }}
    >
      <MenuIconContext.Provider value={hasIcon}>
      {items.map((entry, index) =>
        entry === "separator" ? (
          <MenuDivider key={`separator-${index}`} />
        ) : (
          <MenuItem
            key={entry.id}
            {...(entry.icon ? { icon: entry.icon } : {})}
            {...(entry.shortcut ? { shortcut: entry.shortcut } : {})}
            disabled={entry.disabled}
            data-danger={entry.danger ? "true" : undefined}
            onClick={() => {
              entry.onSelect?.();
              onClose();
            }}
          >
            {entry.label}
          </MenuItem>
        )
      )}
      </MenuIconContext.Provider>
    </div>,
    document.body
  );
}

export interface ContextMenuController {
  open: boolean;
  x: number;
  y: number;
  /** Attach to `onContextMenu` of the target surface. */
  onContextMenu: (event: ReactMouseEvent) => void;
  close: () => void;
}

/** Opens a context menu at the pointer. Pair with {@link ContextMenu}. */
export function useContextMenu(): ContextMenuController {
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  return {
    open: position !== null,
    x: position?.x ?? 0,
    y: position?.y ?? 0,
    onContextMenu: (event) => {
      event.preventDefault();
      setPosition({ x: event.clientX, y: event.clientY });
    },
    close: () => setPosition(null)
  };
}
