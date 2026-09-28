import type { SVGProps } from "react";

/**
 * Official rumahl "r" logomark (the glyph from the rumahl brand wordmark,
 * normalized to a tight viewBox). Fill follows `currentColor` so themes can
 * tint it; the brand default is rumahl green.
 */
export function RumahlMark({ className, ...props }: SVGProps<SVGSVGElement>) {
  return (
    <svg aria-hidden="true" className={className} fill="currentColor" role="img" viewBox="0 0 251 472" {...props}>
      <path d="M 9.47163 471.21342 Q 0 471.21342 0 461.74179 V 203.63996 C 0 80.50881 80.50883 0 201.27207 0 H 241.52649 Q 250.99811 0 250.99811 9.47163 V 94.71626 Q 250.99811 104.18789 241.52649 104.18789 H 210.74370 C 144.44231 104.18789 101.81999 144.44230 101.81999 210.74369 V 461.74179 Q 101.81999 471.21342 92.34836 471.21342 Z" />
    </svg>
  );
}
