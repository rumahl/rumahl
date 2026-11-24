import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
  label?: string;
  /** Element that opened the menu; focus returns here when it closes. */
  restoreFocus?: HTMLElement | null;
}

const VIEWPORT_MARGIN = 8;

function enabledButtons(menu: HTMLElement | null): HTMLButtonElement[] {
  return menu ? Array.from(menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')) : [];
}

/**
 * File action menu. Right-click or the per-item overflow button opens it, the
 * first enabled item takes focus, and arrow/Home/End navigation, Escape, Tab
 * and outside pointer input close it with focus restored to the invoker.
 */
export function ContextMenu({ x, y, items, onClose, label = "File actions", restoreFocus = null }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const menu = ref.current;
    if (!menu) return;
    const invoker = restoreFocus ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    const bounds = menu.getBoundingClientRect();
    const width = bounds.width || menu.offsetWidth;
    const height = bounds.height || menu.offsetHeight;
    setPosition({
      left: Math.max(VIEWPORT_MARGIN, Math.min(x, window.innerWidth - width - VIEWPORT_MARGIN)),
      top: Math.max(VIEWPORT_MARGIN, Math.min(y, window.innerHeight - height - VIEWPORT_MARGIN))
    });

    const close = () => onCloseRef.current();
    const outside = (event: PointerEvent) => {
      if (!menu.contains(event.target as Node)) close();
    };
    document.addEventListener("pointerdown", outside, true);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
      // Restore focus only when the menu still owns it. An action such as
      // Rename may have already moved focus into the inline input.
      const active = document.activeElement;
      const ownsFocus = !active || active === document.body || menu.contains(active);
      if (ownsFocus && invoker?.isConnected) invoker.focus();
    };
  }, [restoreFocus, x, y]);

  // Focus only once the measured position has made the menu visible; browsers
  // refuse to focus an element that is still `visibility: hidden`.
  useEffect(() => {
    if (position) enabledButtons(ref.current)[0]?.focus();
  }, [position]);

  function handleKeys(event: ReactKeyboardEvent<HTMLDivElement>) {
    const buttons = enabledButtons(ref.current);
    if (buttons.length === 0) return;
    const current = document.activeElement instanceof HTMLButtonElement ? buttons.indexOf(document.activeElement) : -1;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key === "Tab") {
      // A menu is not a dialog: Tab closes it instead of escaping into the
      // background while the menu is still visible.
      event.preventDefault();
      onClose();
      return;
    }
    let next: number | null = null;
    if (event.key === "ArrowDown") next = current < 0 ? 0 : (current + 1) % buttons.length;
    if (event.key === "ArrowUp") next = current <= 0 ? buttons.length - 1 : current - 1;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = buttons.length - 1;
    if (next !== null) {
      event.preventDefault();
      event.stopPropagation();
      buttons[next]?.focus();
    }
  }

  const menu = <div
    ref={ref}
    className="files-menu"
    role="menu"
    aria-label={label}
    style={{
      left: position?.left ?? x,
      top: position?.top ?? y,
      visibility: position ? "visible" : "hidden"
    }}
    onKeyDown={handleKeys}
    onPointerDown={(event) => event.stopPropagation()}
    onContextMenu={(event) => event.preventDefault()}>
    {items.map((item) => <button key={item.label} type="button" role="menuitem" disabled={item.disabled} className={item.danger ? "is-danger" : undefined} onClick={() => {
      if (item.disabled) return;
      item.onSelect();
      onClose();
    }}>
      {item.icon}{item.label}
    </button>)}
  </div>;
  return typeof document === "undefined" ? menu : createPortal(menu, document.body);
}
