import { forwardRef, type CSSProperties, type ElementType, type HTMLAttributes, type ReactNode } from "react";
import { space, type Spacing } from "../spacing";

export type FlexAlign = "start" | "center" | "end" | "stretch" | "baseline";
export type FlexJustify = "start" | "center" | "end" | "between" | "around" | "evenly";

const ALIGN: Record<FlexAlign, string> = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  stretch: "stretch",
  baseline: "baseline"
};
const JUSTIFY: Record<FlexJustify, string> = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  between: "space-between",
  around: "space-around",
  evenly: "space-evenly"
};

export interface FlexProps extends HTMLAttributes<HTMLElement> {
  as?: ElementType;
  inline?: boolean;
  direction?: "row" | "column" | "row-reverse" | "column-reverse";
  align?: FlexAlign;
  justify?: FlexJustify;
  wrap?: boolean;
  gap?: Spacing;
  p?: Spacing; px?: Spacing; py?: Spacing;
  m?: Spacing; mx?: Spacing; my?: Spacing;
  children?: ReactNode;
}

/**
 * Flex container with friendly prop names. `gap` uses the 4px scale.
 * Renders any element via `as`.
 */
export const Flex = forwardRef<HTMLElement, FlexProps>(function Flex(
  {
    as: Tag = "div",
    inline,
    direction = "row",
    align,
    justify,
    wrap,
    gap,
    p, px, py,
    m, mx, my,
    className = "",
    style,
    children,
    ...props
  },
  ref
) {
  const layout: CSSProperties = {
    flexDirection: direction,
    ...(align ? { alignItems: ALIGN[align] } : {}),
    ...(justify ? { justifyContent: JUSTIFY[justify] } : {}),
    ...(wrap ? { flexWrap: "wrap" } : {}),
    ...(gap !== undefined ? { gap: space(gap) } : {}),
    ...(p !== undefined ? { padding: space(p) } : {}),
    ...(px !== undefined ? { paddingLeft: space(px), paddingRight: space(px) } : {}),
    ...(py !== undefined ? { paddingTop: space(py), paddingBottom: space(py) } : {}),
    ...(m !== undefined ? { margin: space(m) } : {}),
    ...(mx !== undefined ? { marginLeft: space(mx), marginRight: space(mx) } : {}),
    ...(my !== undefined ? { marginTop: space(my), marginBottom: space(my) } : {})
  };
  return (
    <Tag
      ref={ref}
      data-inline={inline ? "true" : undefined}
      className={`rumahl-flex ${className}`}
      style={{ ...layout, ...style }}
      {...props}
    >
      {children}
    </Tag>
  );
});
