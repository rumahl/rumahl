import type { HTMLAttributes, LiHTMLAttributes, ReactNode } from "react";

export interface ListItemProps extends Omit<LiHTMLAttributes<HTMLLIElement>, "title"> {
  title: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  trailing?: ReactNode;
  /** Makes the whole row a clickable/focusable target. */
  interactive?: boolean;
}

/** A row in a {@link List}. `leading`/`trailing` hold icons, avatars or actions. */
export function ListItem({
  title,
  subtitle,
  leading,
  trailing,
  interactive,
  className = "",
  children,
  ...props
}: ListItemProps) {
  return (
    <li
      className={`rumahl-list-item ${className}`}
      data-interactive={interactive ? "true" : undefined}
      {...(interactive ? { role: "button", tabIndex: 0 } : {})}
      {...props}
    >
      {leading ? <span className="rumahl-list-item__leading">{leading}</span> : null}
      <span className="rumahl-list-item__main">
        <span className="rumahl-list-item__title" style={{ display: "block" }}>
          {title}
        </span>
        {subtitle ? (
          <span className="rumahl-list-item__subtitle" style={{ display: "block" }}>
            {subtitle}
          </span>
        ) : null}
        {children}
      </span>
      {trailing ? <span className="rumahl-list-item__trailing">{trailing}</span> : null}
    </li>
  );
}

/** Semantic list of {@link ListItem}s. */
export function List({ className = "", children, ...props }: HTMLAttributes<HTMLUListElement>) {
  return (
    <ul className={`rumahl-list ${className}`} {...props}>
      {children}
    </ul>
  );
}

/** Divider-friendly list wrapper alias (accepts any list children). */
export function ListGroup(props: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} />;
}
