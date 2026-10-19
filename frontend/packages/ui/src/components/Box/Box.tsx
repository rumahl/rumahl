import { forwardRef, type CSSProperties, type ElementType, type HTMLAttributes, type ReactNode } from "react";
import { space, type Spacing } from "../spacing";

export interface BoxProps extends HTMLAttributes<HTMLElement> {
  /** Element to render (default `div`). */
  as?: ElementType;
  p?: Spacing; px?: Spacing; py?: Spacing;
  pt?: Spacing; pr?: Spacing; pb?: Spacing; pl?: Spacing;
  m?: Spacing; mx?: Spacing; my?: Spacing;
  mt?: Spacing; mr?: Spacing; mb?: Spacing; ml?: Spacing;
  gap?: Spacing;
  background?: string;
  radius?: Spacing;
  border?: boolean;
  children?: ReactNode;
}

/**
 * Generic layout container. Spacing props use a 4px scale (`p={2}` → `8px`) and
 * raw CSS lengths pass through. Renders any element via `as`.
 *
 * Tokens: inherits `--control-*`; `border` uses `--control-border`, `background`
 * and `radius` accept any CSS value.
 */
export const Box = forwardRef<HTMLElement, BoxProps>(function Box(
  {
    as: Tag = "div",
    p, px, py, pt, pr, pb, pl,
    m, mx, my, mt, mr, mb, ml,
    gap, background, radius, border,
    className = "",
    style,
    children,
    ...props
  },
  ref
) {
  const layout: CSSProperties = {
    ...(p !== undefined ? { padding: space(p) } : {}),
    ...(px !== undefined ? { paddingLeft: space(px), paddingRight: space(px) } : {}),
    ...(py !== undefined ? { paddingTop: space(py), paddingBottom: space(py) } : {}),
    ...(pt !== undefined ? { paddingTop: space(pt) } : {}),
    ...(pr !== undefined ? { paddingRight: space(pr) } : {}),
    ...(pb !== undefined ? { paddingBottom: space(pb) } : {}),
    ...(pl !== undefined ? { paddingLeft: space(pl) } : {}),
    ...(m !== undefined ? { margin: space(m) } : {}),
    ...(mx !== undefined ? { marginLeft: space(mx), marginRight: space(mx) } : {}),
    ...(my !== undefined ? { marginTop: space(my), marginBottom: space(my) } : {}),
    ...(mt !== undefined ? { marginTop: space(mt) } : {}),
    ...(mr !== undefined ? { marginRight: space(mr) } : {}),
    ...(mb !== undefined ? { marginBottom: space(mb) } : {}),
    ...(ml !== undefined ? { marginLeft: space(ml) } : {}),
    ...(gap !== undefined ? { gap: space(gap) } : {}),
    ...(background !== undefined ? { background } : {}),
    ...(radius !== undefined ? { borderRadius: space(radius) } : {}),
    ...(border ? { border: "1px solid var(--control-border)" } : {})
  };
  return (
    <Tag ref={ref} className={`rumahl-box ${className}`} style={{ ...layout, ...style }} {...props}>
      {children}
    </Tag>
  );
});
