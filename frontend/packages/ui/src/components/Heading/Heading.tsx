import type { ElementType, HTMLAttributes, ReactNode } from "react";
import type { ControlSize } from "../../size";

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

const DEFAULT_SIZE: Record<HeadingLevel, ControlSize> = {
  1: "xl",
  2: "lg",
  3: "md",
  4: "sm",
  5: "xs",
  6: "xs"
};

export interface HeadingProps extends HTMLAttributes<HTMLHeadingElement> {
  as?: ElementType;
  /** Semantic level; also picks the default visual size. */
  level?: HeadingLevel;
  size?: ControlSize | undefined;
  tone?: "default" | "muted";
  children?: ReactNode;
}

/**
 * Heading primitive. `level` sets the semantic `<h1>`–`<h6>` and the default
 * visual size; override with `size` when the outline and the visuals differ.
 */
export function Heading({
  as,
  level = 2,
  size,
  tone = "default",
  className = "",
  children,
  ...props
}: HeadingProps) {
  const Tag = as ?? (`h${level}` as ElementType);
  return (
    <Tag
      data-size={size ?? DEFAULT_SIZE[level]}
      data-tone={tone === "default" ? undefined : tone}
      className={`rumahl-heading ${className}`}
      {...props}
    >
      {children}
    </Tag>
  );
}
