import type { HTMLAttributes } from "react";

export interface DividerProps extends HTMLAttributes<HTMLHRElement> {
  orientation?: "horizontal" | "vertical";
}

/**
 * Hairline separator between sections or inline items.
 *
 * Tokens: `--control-border`.
 */
export function Divider({ orientation = "horizontal", className = "", ...props }: DividerProps) {
  return (
    <hr
      role="separator"
      aria-orientation={orientation}
      data-orientation={orientation}
      className={`rumahl-divider ${className}`}
      {...props}
    />
  );
}
