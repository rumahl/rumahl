import type { ElementType, HTMLAttributes, ReactNode } from "react";
import type { ControlSize } from "../../size";

export type TextTone = "default" | "muted" | "accent" | "success" | "warning" | "danger";
export type TextWeight = "regular" | "medium" | "semibold" | "bold";

export interface TextProps extends HTMLAttributes<HTMLElement> {
  as?: ElementType;
  size?: ControlSize | undefined;
  tone?: TextTone;
  weight?: TextWeight;
  truncate?: boolean;
  children?: ReactNode;
}

/**
 * Typography primitive. `size` uses the shared scale (`xs`–`xl`), `tone` maps
 * to colour roles, `truncate` adds a single-line ellipsis. Renders any element
 * via `as`.
 *
 * Tokens: `--rumahl-ui-color-text-primary/-muted/-accent`, accent/danger roles.
 */
export function Text({
  as: Tag = "span",
  size = "md",
  tone = "default",
  weight,
  truncate,
  className = "",
  children,
  ...props
}: TextProps) {
  return (
    <Tag
      data-size={size}
      data-tone={tone === "default" ? undefined : tone}
      data-weight={weight}
      data-truncate={truncate ? "true" : undefined}
      className={`rumahl-text ${className}`}
      {...props}
    >
      {children}
    </Tag>
  );
}
