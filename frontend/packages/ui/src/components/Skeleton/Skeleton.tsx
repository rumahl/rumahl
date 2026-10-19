import type { CSSProperties } from "react";

export interface SkeletonProps {
  variant?: "rect" | "text" | "circle";
  width?: number | string;
  height?: number | string;
  /** Render a paragraph of N shimmering lines (text variant). */
  lines?: number;
  className?: string;
}

const toLength = (value?: number | string): string | undefined =>
  typeof value === "number" ? `${value}px` : value;

/** Loading placeholder. Mark the container busy; skeletons are decorative. */
export function Skeleton({ variant = "rect", width, height, lines, className = "" }: SkeletonProps) {
  if (lines && lines > 1) {
    return (
      <div className={`rumahl-skeleton__lines ${className}`} aria-hidden="true">
        {Array.from({ length: lines }, (_, index) => (
          <div
            key={index}
            className="rumahl-skeleton"
            data-variant="text"
            style={{ width: index === lines - 1 ? "70%" : "100%" }}
          />
        ))}
      </div>
    );
  }
  const style: CSSProperties = { width: toLength(width), height: toLength(height) };
  return <div className={`rumahl-skeleton ${className}`} data-variant={variant} style={style} aria-hidden="true" />;
}
