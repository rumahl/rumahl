import type { ButtonHTMLAttributes, CSSProperties, ElementType, ReactNode } from "react";
import type { ControlSize } from "../../size";
import { colorStyle, onColor, type ColorProps } from "../../tint";
import { RumahlTooltip } from "../RumahlTooltip/RumahlTooltip";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  /** Accessible name (required — icon-only). */
  label: string;
  /** Element to render (default `button`; use `a` for links). */
  as?: ElementType;
  /** Link props used when `as="a"`. */
  href?: string;
  target?: string;
  rel?: string;
  shape?: "square" | "circle";
  variant?: "secondary" | "primary" | "ghost";
  size?: ControlSize | undefined;
  /** Explicit fill colour (independent of the text/accent `color`). */
  backgroundColor?: string;
  /** Shows a spinner and blocks interaction. */
  loading?: boolean;
  /** Element state: float over a background (image, another surface). */
  floating?: boolean;
  /** Persistent active/selected state. */
  active?: boolean;
  /** Tooltip text (native `title` is also supported). */
  title?: string;
  children?: ReactNode;
}

/**
 * Icon-only button. Always pass `label` for the accessible name; `title` adds a
 * tooltip. Supports `loading`, `active`, `color` and rendering as a link via
 * `as`. All native button props forward.
 *
 * Shares the control material and the shared size scale (`xs`–`xl`).
 * Tokens: `--control-fill/-border/-inset/-filter`, `--rumahl-ui-color-accent`.
 */
export function IconButton({
  label,
  as: Tag = "button",
  shape = "square",
  variant = "secondary",
  size = "md",
  loading,
  floating,
  active,
  title,
  type = "button",
  className = "",
  color,
  backgroundColor,
  disabled,
  style,
  children,
  ...props
}: IconButtonProps) {
  const button = (
    <Tag
      {...(Tag === "button" ? { type, disabled: disabled || loading } : {})}
      aria-label={label}
      aria-busy={loading ? true : undefined}
      data-shape={shape}
      data-variant={variant === "secondary" ? undefined : variant}
      data-size={size}
      data-floating={floating ? "" : undefined}
      data-active={active ? "true" : undefined}
      data-loading={loading ? "true" : undefined}
      data-bg={backgroundColor ? "true" : undefined}
      className={`rumahl-icon-button ${className}`}
      style={
        {
          ...colorStyle(color),
          ...(backgroundColor
            ? { "--btn-bg": backgroundColor, "--btn-bg-ink": onColor(backgroundColor) }
            : {}),
          ...style
        } as CSSProperties
      }
      {...props}
    >
      {loading ? <span className="rumahl-icon-button__loader" aria-hidden="true" /> : children}
    </Tag>
  );
  return title ? <RumahlTooltip content={title}>{button}</RumahlTooltip> : button;
}
