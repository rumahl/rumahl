import type { AnchorHTMLAttributes, ElementType, ReactNode } from "react";

/** Vertical navigation rail: an optional identity header, sections and footer. */
export function Sidenav({
  header,
  children,
  footer,
  className = ""
}: {
  header?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
}) {
  return (
    <aside className={`rumahl-sidenav ${className}`}>
      {header}
      <nav className="rumahl-sidenav__nav">{children}</nav>
      {footer ? <div className="rumahl-sidenav__footer">{footer}</div> : null}
    </aside>
  );
}

/** A titled group of sidenav items. */
export function SidenavSection({ title, children }: { title?: ReactNode; children: ReactNode }) {
  return (
    <div className="rumahl-sidenav__section">
      {title ? <div className="rumahl-sidenav__heading">{title}</div> : null}
      {children}
    </div>
  );
}

export interface SidenavIdentityProps {
  name: string;
  detail?: ReactNode;
  /** Overrides the avatar initial. */
  avatar?: string;
  className?: string;
}

/** Signed-in identity header for a {@link Sidenav}. */
export function SidenavIdentity({ name, detail, avatar, className = "" }: SidenavIdentityProps) {
  const initial = (avatar ?? name.trim().slice(0, 1).toLocaleUpperCase()) || "?";
  return (
    <div className={`rumahl-sidenav__identity ${className}`}>
      <span className="rumahl-sidenav__avatar" aria-hidden="true">
        {initial}
      </span>
      <div>
        <strong>{name}</strong>
        {detail ? <span>{detail}</span> : null}
      </div>
    </div>
  );
}

export interface SidenavItemProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  as?: ElementType;
  icon?: ReactNode;
  /** Marks the active page (`aria-current`). */
  current?: boolean;
  /** When rendered `as="button"`. */
  type?: "button" | "submit" | "reset";
  children: ReactNode;
}

/** Sidenav navigation item. Use `as` for router links. */
export function SidenavItem({
  as: Tag = "a",
  icon,
  current,
  className = "",
  children,
  ...props
}: SidenavItemProps) {
  return (
    <Tag className={`rumahl-sidenav__item ${className}`} aria-current={current ? "page" : undefined} {...props}>
      {icon ? (
        <span className="rumahl-sidenav__icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span className="rumahl-sidenav__label">{children}</span>
    </Tag>
  );
}
