import { Children, type ReactNode } from "react";

export interface AvatarGroupProps {
  children: ReactNode;
  /** Show at most this many, collapsing the rest into a “+N” badge. */
  max?: number;
  className?: string;
}

/** Overlapping avatars with an optional “+N” overflow badge. */
export function AvatarGroup({ children, max, className = "" }: AvatarGroupProps) {
  const items = Children.toArray(children);
  const shown = typeof max === "number" ? items.slice(0, max) : items;
  const extra = items.length - shown.length;
  return (
    <div className={`rumahl-avatar-group ${className}`}>
      {shown}
      {extra > 0 ? <span className="rumahl-avatar-group__more">+{extra}</span> : null}
    </div>
  );
}
