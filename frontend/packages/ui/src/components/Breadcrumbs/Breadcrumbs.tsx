import { Fragment, type ReactNode } from "react";

export interface Crumb {
  label: ReactNode;
  href?: string;
  onClick?: () => void;
}

export interface BreadcrumbsProps {
  items: readonly Crumb[];
  label?: string;
  className?: string;
}

/** Navigation trail; the last item is marked `aria-current="page"`. */
export function Breadcrumbs({ items, label = "Breadcrumb", className = "" }: BreadcrumbsProps) {
  return (
    <nav aria-label={label} className={`rumahl-breadcrumbs ${className}`}>
      <ol style={{ display: "contents", listStyle: "none", margin: 0, padding: 0 }}>
        {items.map((item, index) => {
          const last = index === items.length - 1;
          return (
            <Fragment key={index}>
              {index > 0 ? (
                <span className="rumahl-breadcrumbs__sep" aria-hidden="true">
                  /
                </span>
              ) : null}
              <li style={{ display: "contents" }}>
                {item.href ? (
                  <a
                    className="rumahl-breadcrumbs__item"
                    href={item.href}
                    aria-current={last ? "page" : undefined}
                    onClick={item.onClick}
                  >
                    {item.label}
                  </a>
                ) : (
                  <span className="rumahl-breadcrumbs__item" aria-current={last ? "page" : undefined}>
                    {item.label}
                  </span>
                )}
              </li>
            </Fragment>
          );
        })}
      </ol>
    </nav>
  );
}
