import type { HTMLAttributes, ReactNode } from "react";

/** App toolbar container. Compose with `IconButton` and `ToolbarGroup`. */
export function Toolbar({
  className = "",
  floating,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & { floating?: boolean | undefined }) {
  return (
    <div role="toolbar" className={`rumahl-toolbar ${className}`} data-floating={floating ? "" : undefined} {...props}>
      {children}
    </div>
  );
}

/** Groups related toolbar buttons. */
export function ToolbarGroup({ className = "", children }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`rumahl-toolbar__group ${className}`}>{children}</div>;
}

/** Pushes following content to the end. */
export function ToolbarSpacer() {
  return <span className="rumahl-toolbar__spacer" aria-hidden="true" />;
}

export interface ToolbarSeparatorProps {
  className?: string;
}

/** Thin vertical rule between toolbar groups. */
export function ToolbarSeparator({ className = "" }: ToolbarSeparatorProps): ReactNode {
  return <span className={`rumahl-divider ${className}`} data-orientation="vertical" aria-hidden="true" />;
}
