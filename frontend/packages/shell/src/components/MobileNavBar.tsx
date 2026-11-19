import type { ReactNode } from "react";

/**
 * iOS-style navigation bar for phone app windows. Rendered in the DOM on every
 * viewport but only displayed by the mobile shell; desktop keeps its existing
 * titlebar and page header.
 */
export function MobileNavBar({ title, back, leading, actions, heading = false }: {
  title: string;
  /** Back affordance (usually a link). Takes precedence over `leading`. */
  back?: ReactNode;
  /** Optional leading control when there is no back affordance. */
  leading?: ReactNode;
  /** Trailing actions; each should be a 44px icon button. */
  actions?: ReactNode;
  /** Render the visible title as the page's h1 (detail pages without a large title). */
  heading?: boolean;
}) {
  const Title = heading ? "h1" : "span";
  return <header className="mobile-nav-bar">
    <div className="mobile-nav-bar__leading">{back ?? leading}</div>
    <Title className="mobile-nav-bar__title">{title}</Title>
    <div className="mobile-nav-bar__actions">{actions}</div>
  </header>;
}

/** iOS large title below the navigation bar, shown on root app screens. */
export function MobileLargeTitle({ title, subtitle }: { title: string; subtitle?: string | undefined }) {
  return <div className="mobile-large-title">
    <h1 className="mobile-large-title__title">{title}</h1>
    {subtitle ? <p className="mobile-large-title__subtitle">{subtitle}</p> : null}
  </div>;
}
