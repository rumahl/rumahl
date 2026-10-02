import type { PropsWithChildren } from "react";
export function DesktopLayout({ children }: PropsWithChildren) {
  return <div className="desktop-workspace"><div className="desktop-orbit" aria-hidden="true" />{children}</div>;
}
