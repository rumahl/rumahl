import type { PropsWithChildren } from "react";
export function LauncherLayout({ children }: PropsWithChildren) {
  return <div className="launcher-workspace">{children}</div>;
}
