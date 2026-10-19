import type { ElementType, HTMLAttributes, ReactNode } from "react";

export interface HiddenTextProps extends HTMLAttributes<HTMLElement> {
  as?: ElementType;
  children?: ReactNode;
}

/**
 * Visually hidden but available to assistive technology. Use for extra context
 * a sighted user reads from layout (e.g. "opens in a new tab").
 */
export function HiddenText({ as: Tag = "span", className = "", children, ...props }: HiddenTextProps) {
  return (
    <Tag className={`rumahl-hidden-text ${className}`} {...props}>
      {children}
    </Tag>
  );
}
