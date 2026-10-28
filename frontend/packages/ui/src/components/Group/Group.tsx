import type { AnchorHTMLAttributes, CSSProperties, ElementType, ReactNode } from "react";
import { ExternalLinkIcon } from "../../icons";
import { colorStyle, type ColorProps } from "../../tint";

/*
 * Native grouping containers used across the OS and apps. A `GroupContainer`
 * holds rows (label + control/value); a `GroupLinkContainer` holds drill-in
 * `GroupLink` rows. Both are plain cards on the shared control material.
 */

export interface GroupContainerProps {
  children: ReactNode;
  /** Element state: float over a background (image, another surface). */
  floating?: boolean;
  className?: string;
  style?: CSSProperties;
}

/** Card that groups content rows. */
export function GroupContainer({ children, floating, className = "", style }: GroupContainerProps) {
  return (
    <div className={`rumahl-group ${className}`} data-floating={floating ? "" : undefined} style={style}>
      {children}
    </div>
  );
}

export interface GroupRowProps {
  title: ReactNode;
  description?: ReactNode;
  /** Read-only value (right-aligned, muted). */
  value?: ReactNode;
  /** Control shown on the right (switch, select, button…). */
  children?: ReactNode;
  className?: string;
}

/** One labeled row inside a `GroupContainer`. */
export function GroupRow({ title, description, value, children, className = "" }: GroupRowProps) {
  return (
    <div
      className={`rumahl-group__row ${className}`}
      data-variant={value !== undefined && children === undefined ? "value" : undefined}
    >
      <div className="rumahl-group__copy">
        <strong>{title}</strong>
        {description ? <p>{description}</p> : null}
      </div>
      <div className="rumahl-group__visual">
        {value !== undefined ? <span>{value}</span> : children}
      </div>
    </div>
  );
}

export interface GroupLinkContainerProps {
  children: ReactNode;
  className?: string;
}

/** Card that groups navigation links. */
export function GroupLinkContainer({ children, className = "" }: GroupLinkContainerProps) {
  return <div className={`rumahl-group rumahl-group--links ${className}`}>{children}</div>;
}

export interface GroupLinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "title">, ColorProps {
  as?: ElementType;
  icon?: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  /** Opens in a new tab with a safe rel and an external glyph. */
  external?: boolean;
  /** When rendered `as="button"`. */
  type?: "button" | "submit" | "reset";
}

/** Drill-in navigation row; a chevron for internal, a glyph for external. */
export function GroupLink({
  as: Tag = "a",
  icon,
  title,
  subtitle,
  external,
  className = "",
  color,
  style,
  rel,
  target,
  ...props
}: GroupLinkProps) {
  return (
    <Tag
      className={`rumahl-group__link ${className}`}
      style={{ ...colorStyle(color), ...style }}
      {...(external ? { target: target ?? "_blank", rel: rel ?? "noreferrer noopener" } : {})}
      {...props}
    >
      {icon ? (
        <span className="rumahl-group__link-icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span className="rumahl-group__link-text">
        <strong>{title}</strong>
        {subtitle ? <small>{subtitle}</small> : null}
      </span>
      {external ? (
        <ExternalLinkIcon className="rumahl-group__link-ext" aria-hidden="true" />
      ) : (
        <span className="rumahl-group__link-chevron" aria-hidden="true">
          ›
        </span>
      )}
    </Tag>
  );
}
