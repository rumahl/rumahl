import type { HTMLAttributes, SVGProps } from "react";
import "./RumahlMark.css";

export interface RumahlMarkProps extends SVGProps<SVGSVGElement> {
  /**
   * - `solid` (default) — filled with `currentColor`.
   * - `glass` — the mark is **made of the glass control material** (recess +
   *   fill + blur), shaped by the glyph. Renders a `<span>` (size via `style`),
   *   so it matches a secondary button / glass input exactly.
   * - `engraved` — the mark itself with **no fill colour**: only the cut edge.
   */
  variant?: "solid" | "glass" | "engraved";
}

const R_PATH =
  "M 9.47163 471.21342 Q 0 471.21342 0 461.74179 V 203.63996 C 0 80.50881 80.50883 0 201.27207 0 H 241.52649 Q 250.99811 0 250.99811 9.47163 V 94.71626 Q 250.99811 104.18789 241.52649 104.18789 H 210.74370 C 144.44231 104.18789 101.81999 144.44230 101.81999 210.74369 V 461.74179 Q 101.81999 471.21342 92.34836 471.21342 Z";

/**
 * Official rumahl "r" logomark (the glyph from the rumahl brand wordmark,
 * normalized to a tight viewBox). Fill follows `currentColor` so themes can
 * tint it; the brand default is rumahl green.
 */
export function RumahlMark({ className = "", variant = "solid", style, ...props }: RumahlMarkProps) {
  if (variant === "glass") {
    return (
      <span
        aria-hidden="true"
        className={`rumahl-mark rumahl-mark--glass${className ? ` ${className}` : ""}`}
        style={style as HTMLAttributes<HTMLElement>["style"]}
        {...(props as HTMLAttributes<HTMLElement>)}
      />
    );
  }
  const classes = `rumahl-mark${variant === "engraved" ? " rumahl-mark--engraved" : ""}${className ? ` ${className}` : ""}`;
  return (
    <svg aria-hidden="true" className={classes} fill="currentColor" role="img" viewBox="0 0 251 472" {...props}>
      <path d={R_PATH} />
    </svg>
  );
}
