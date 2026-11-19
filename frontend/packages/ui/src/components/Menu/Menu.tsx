import { Children, createContext, isValidElement, useContext, type ButtonHTMLAttributes, type HTMLAttributes, type ReactNode } from "react";
import { colorStyle, type ColorProps } from "../../tint";

/** True when a menu reserves an icon column (any item has an icon). */
export const MenuIconContext = createContext(false);

export interface MenuProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  /** Force the icon column on/off; otherwise inferred from the items. */
  iconColumn?: boolean;
}

/** Popup menu container (`role="menu"`). */
export function Menu({ className = "", iconColumn, children, ...props }: MenuProps) {
  let icons = iconColumn ?? false;
  if (iconColumn === undefined) {
    Children.forEach(children, (child) => {
      if (isValidElement(child) && (child.props as { icon?: unknown }).icon != null) icons = true;
    });
  }
  return (
    <div role="menu" className={`rumahl-menu ${className}`} {...props}>
      <MenuIconContext.Provider value={icons}>{children}</MenuIconContext.Provider>
    </div>
  );
}

export interface MenuItemProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  icon?: ReactNode;
  shortcut?: string;
  /** For checkable items; sets `menuitemcheckbox` + `aria-checked`. */
  selected?: boolean;
  children: ReactNode;
}

/** A single menu row. `selected` makes it a checkable menu item. */
export function MenuItem({
  icon,
  shortcut,
  selected,
  className = "",
  color,
  style,
  children,
  role,
  ...props
}: MenuItemProps) {
  const reserveIcon = useContext(MenuIconContext);
  return (
    <button
      type="button"
      role={role ?? (selected !== undefined ? "menuitemcheckbox" : "menuitem")}
      aria-checked={selected !== undefined ? selected : undefined}
      className={`rumahl-menu-item ${className}`}
      style={{ ...colorStyle(color), ...style }}
      {...props}
    >
      {reserveIcon || icon ? (
        <span className="rumahl-menu-item__icon" aria-hidden={icon ? undefined : true}>
          {icon}
        </span>
      ) : null}
      <span className="rumahl-menu-item__label">{children}</span>
      {shortcut ? <span className="rumahl-menu-item__shortcut">{shortcut}</span> : null}
    </button>
  );
}

/** Separator between menu groups. */
export function MenuDivider() {
  return <div className="rumahl-menu-divider" role="separator" />;
}
