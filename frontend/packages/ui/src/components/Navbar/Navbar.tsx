import type { AnchorHTMLAttributes, ElementType, ReactNode } from "react";

export interface NavbarProps {
  brand?: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
  label?: string;
  className?: string;
}

/** Horizontal navigation bar: brand, items and trailing actions. */
export function Navbar({ brand, children, actions, label = "Primary", className = "" }: NavbarProps) {
  return (
    <nav aria-label={label} className={`rumahl-navbar ${className}`}>
      {brand ? <div className="rumahl-navbar__brand">{brand}</div> : null}
      <div className="rumahl-navbar__items">{children}</div>
      {actions ? <div className="rumahl-navbar__actions">{actions}</div> : null}
    </nav>
  );
}

export interface NavbarItemProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  as?: ElementType;
  icon?: ReactNode;
  current?: boolean;
  type?: "button" | "submit" | "reset";
  children: ReactNode;
}

/** Navbar navigation item. Use `as` for router links. */
export function NavbarItem({
  as: Tag = "a",
  icon,
  current,
  className = "",
  children,
  ...props
}: NavbarItemProps) {
  return (
    <Tag className={`rumahl-navbar__item ${className}`} aria-current={current ? "page" : undefined} {...props}>
      {icon ? (
        <span className="rumahl-navbar__icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span>{children}</span>
    </Tag>
  );
}
