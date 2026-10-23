import type { ReactNode } from "react";

export interface MenuBarMenu {
  id: string;
  label: string;
}

export interface MenuBarProps {
  /** Left brand/mark. */
  brand?: ReactNode;
  /** Leading menus (File, Edit, …). */
  menus?: readonly MenuBarMenu[];
  onSelect?: (id: string) => void;
  /** Right status cluster (tray icons). */
  status?: ReactNode;
  /** Clock text. */
  clock?: string;
  label?: string;
  className?: string;
}

/**
 * System menu bar — **protected** OS chrome. Two floating top-bar islands
 * (left: brand + menus, right: status + clock) on the rumahl island recipe
 * (theme/material aware). Pure CSS: no glass engine required, so it looks the
 * same in the OS, apps and Storybook.
 */
export function MenuBar({
  brand,
  menus = [],
  onSelect,
  status,
  clock,
  label = "Menu bar",
  className = ""
}: MenuBarProps) {
  return (
    <div className={`rumahl-menubar ${className}`} role="toolbar" aria-label={label} data-system="menubar">
      <div className="rumahl-menubar__island">
        {brand ? <span className="rumahl-menubar__brand">{brand}</span> : null}
        {menus.map((menu) => (
          <button key={menu.id} type="button" className="rumahl-menubar__item" onClick={() => onSelect?.(menu.id)}>
            {menu.label}
          </button>
        ))}
      </div>
      <div className="rumahl-menubar__island rumahl-menubar__island--right">
        {status}
        {clock ? <span className="rumahl-menubar__clock">{clock}</span> : null}
      </div>
    </div>
  );
}
