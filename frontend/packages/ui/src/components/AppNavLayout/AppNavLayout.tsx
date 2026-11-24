import type { ReactNode, Ref } from "react";
import { MobileIslandNav, type MobileIslandNavItem } from "../MobileIslandNav/MobileIslandNav";
import { useRegisterMobileNav } from "../MobileIslandNav/MobileNavSlot";

export interface AppNavLayoutMobileNav {
  items: readonly MobileIslandNavItem[];
  value: string;
  onChange: (id: string) => void;
  label: string;
  className?: string;
}

export interface AppNavLayoutProps {
  /** Optional top navigation. Can live outside the layout, e.g. in a titlebar. */
  navbar?: ReactNode;
  /** Desktop navigation rail (typically a `Sidenav`). */
  sidebar?: ReactNode;
  /** Standard narrow-window navigation, shown instead of the rail below 760px. */
  mobileNav?: AppNavLayoutMobileNav;
  children: ReactNode;
  className?: string;
  contentClassName?: string;
  contentRef?: Ref<HTMLDivElement>;
}

/**
 * Navbar + sidenav application frame. The sidebar is the desktop navigation;
 * below 760px it is replaced by the standard draggable mobile island. Use the
 * navbar slot when the layout owns the top bar; titlebar-hosted navbars may stay
 * outside the frame.
 */
export function AppNavLayout({
  navbar,
  sidebar,
  mobileNav,
  children,
  className = "",
  contentClassName = "",
  contentRef
}: AppNavLayoutProps) {
  // Inside the shell the bottom bar is owned by the window (system actions +
  // registered app destinations). Outside it (Storybook, UI tests) the layout
  // still renders the standalone island.
  const slotted = useRegisterMobileNav(mobileNav ?? null);
  return <div className={`rumahl-app-nav-layout ${className}`.trim()} data-mobile-nav-slotted={slotted ? "true" : undefined}>
    {navbar ? <div className="rumahl-app-nav-layout__navbar">{navbar}</div> : null}
    <div className={`rumahl-app-nav-layout__body${sidebar ? "" : " is-content-only"}`}>
      {sidebar ? <div className="rumahl-app-nav-layout__rail">{sidebar}</div> : null}
      <div className={`rumahl-app-nav-layout__content ${contentClassName}`.trim()} ref={contentRef}>{children}</div>
    </div>
    {mobileNav && !slotted ? <div className="rumahl-app-nav-layout__island"><MobileIslandNav {...mobileNav} /></div> : null}
  </div>;
}
