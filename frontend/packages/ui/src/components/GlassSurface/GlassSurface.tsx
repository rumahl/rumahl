import type { CSSProperties, HTMLAttributes, ReactNode } from "react";
import { GLASS_HOST_CLASS, GLASS_SURFACE_CLASS } from "../../glass";

export type GlassMaterialName = "bar" | "dock" | "widget";

export interface GlassSurfaceProps extends HTMLAttributes<HTMLDivElement> {
  /** Named material: `bar` (toolbars), `dock` or `widget`. */
  material?: GlassMaterialName;
  /** Corner radius in px (defaults to the material's radius). */
  radius?: number;
  children?: ReactNode;
}

/**
 * A glass surface using the rumahl glass contract.
 *
 * **Only rumahl OS renders true glass.** The real refraction glass is a shell
 * capability (SVG/WebGL engine that samples the wallpaper); it runs inside the
 * OS only. In apps, the extracted UI and Storybook, `@rumahl/ui/glass.css`
 * renders the same look without refraction. On the desktop the shell upgrades
 * this surface to real glass automatically.
 */
export function GlassSurface({
  material = "bar",
  radius,
  className = "",
  style,
  children,
  ...props
}: GlassSurfaceProps) {
  const vars: CSSProperties = radius !== undefined ? { borderRadius: radius } : {};
  return (
    <div
      className={`${GLASS_HOST_CLASS} ${className}`}
      data-glass-material={material}
      style={{ ...vars, ...style }}
      {...props}
    >
      <span className="rumahl-glass-frame" data-glass-fallback aria-hidden="true">
        <span className={GLASS_SURFACE_CLASS} />
      </span>
      {children}
    </div>
  );
}
