import { useState, type ReactNode } from "react";
import { Sidenav } from "../Sidenav/Sidenav";

/*
 * Application layout primitives. `AppLayout` is the sidebar-based layout (one
 * of several); `PageHeader` and `PageSection` frame the content. The sidebar
 * itself is the native `Sidenav`. The older `Settings*` names are kept as
 * aliases.
 */

export interface AppLayoutProps {
  /** Sidebar rail (typically a `Sidenav`). Omit for a content-only layout. */
  sidebar?: ReactNode;
  children: ReactNode;
  /** Controlled collapsed state. Leave undefined for uncontrolled. */
  collapsed?: boolean | undefined;
  /** Initial collapsed state when uncontrolled. */
  defaultCollapsed?: boolean | undefined;
  /** Shows a button that collapses/expands the sidebar. */
  collapsible?: boolean;
  onCollapsedChange?: (collapsed: boolean) => void;
  className?: string;
}

/** Two-pane application layout: a navigation rail beside the content area. */
export function AppLayout({
  sidebar,
  children,
  collapsed,
  defaultCollapsed = false,
  collapsible,
  onCollapsedChange,
  className = ""
}: AppLayoutProps) {
  const [internal, setInternal] = useState(defaultCollapsed);
  const isCollapsed = collapsed ?? internal;
  const toggle = () => {
    const next = !isCollapsed;
    if (collapsed === undefined) setInternal(next);
    onCollapsedChange?.(next);
  };
  return (
    <div className={`rumahl-settings rumahl-window-surface ${className}`} data-collapsed={isCollapsed ? "true" : undefined}>
      {sidebar ? <div className="rumahl-settings__rail">{sidebar}</div> : null}
      {sidebar && collapsible ? (
        <button
          type="button"
          className="rumahl-settings__toggle"
          aria-label={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!isCollapsed}
          onClick={toggle}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="m15 6-6 6 6 6" />
          </svg>
        </button>
      ) : null}
      <div className="rumahl-settings__content">{children}</div>
    </div>
  );
}

export interface AppSidebarProps {
  /** Optional header — any content (search, dropdown, identity …) or nothing. */
  header?: ReactNode;
  children: ReactNode;
  /** Optional footer — any content, or nothing. */
  footer?: ReactNode;
  className?: string;
}

/** Navigation rail for an `AppLayout` (thin wrapper over `Sidenav`). */
export function AppSidebar({ header, children, footer, className = "" }: AppSidebarProps) {
  return (
    <Sidenav header={header} footer={footer} className={className}>
      {children}
    </Sidenav>
  );
}

export interface PageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  eyebrow?: ReactNode;
  /** Back affordance shown before the title. */
  back?: ReactNode;
  children?: ReactNode;
  className?: string;
}

/** Content page: a heading (with optional back link) above grouped sections. */
export function PageHeader({ title, description, eyebrow, back, children, className = "" }: PageHeaderProps) {
  return (
    <section className={`rumahl-settings__page ${className}`}>
      <header className="rumahl-settings__page-heading">
        {back ? <span className="rumahl-settings__back">{back}</span> : null}
        <div>
          {eyebrow ? <span className="rumahl-settings__eyebrow">{eyebrow}</span> : null}
          <h1>{title}</h1>
          {description ? <p>{description}</p> : null}
        </div>
      </header>
      {children}
    </section>
  );
}

/** Titled section within a page. */
export function PageSection({
  title,
  children,
  className = ""
}: {
  title?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rumahl-settings__section ${className}`}>
      {title ? <h3 className="rumahl-settings__section-title">{title}</h3> : null}
      {children}
    </section>
  );
}

/** Small muted helper text under a section. */
export function PageFootnote({ children }: { children: ReactNode }) {
  return <p className="rumahl-settings__footnote">{children}</p>;
}

/* Backwards-compatible `Settings*` aliases. */
export const SettingsLayout = AppLayout;
export const SettingsSidebar = AppSidebar;
export const SettingsPage = PageHeader;
export const SettingsSection = PageSection;
export const SettingsFootnote = PageFootnote;

/* Native building blocks re-exported under the Settings names. */
export {
  SidenavIdentity as SettingsIdentity,
  SidenavItem as SettingsNavItem,
  SidenavSection as SettingsNavSection
} from "../Sidenav/Sidenav";
export {
  GroupContainer as SettingsGroup,
  GroupRow as SettingRow,
  GroupLink as SettingsNavRow
} from "../Group/Group";
