import type { HTMLAttributes, ReactNode } from "react";

export interface AppGridProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  /** Fixed column count; omit for an auto-fill grid. */
  columns?: number;
}

/**
 * Grid that arranges `AppTile`s (launcher / desktop). Flat by design, like the
 * tiles themselves — no glass/material.
 */
export function AppGrid({ children, columns, className = "", style, ...props }: AppGridProps) {
  return (
    <div
      className={`rumahl-app-grid ${className}`}
      style={{ ...(columns ? { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` } : {}), ...style }}
      {...props}
    >
      {children}
    </div>
  );
}
