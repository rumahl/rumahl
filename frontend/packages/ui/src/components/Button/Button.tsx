import { RumahlTooltip } from "../RumahlTooltip/RumahlTooltip";
import type { ControlSize } from "../../size";
import { colorStyle, onColor, type ColorProps } from "../../tint";
import type { AnchorHTMLAttributes, ButtonHTMLAttributes, CSSProperties, ReactNode } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = ControlSize;

/** Shared class names so links and buttons look identical. */
export function buttonClass(variant: ButtonVariant = "secondary", size: ButtonSize = "md", extra?: string): string {
  return ["btn", `btn--${variant}`, `btn--${size}`, extra].filter(Boolean).join(" ");
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Explicit fill colour (independent of the text/accent `color`). */
  backgroundColor?: string;
  /** Element state: float over a background (image, another surface). */
  floating?: boolean;
  /** Override: `glass={false}` renders a flat button without glass. */
  glass?: boolean;
  /** Leading icon. */
  leftIcon?: ReactNode;
  /** Trailing icon. */
  rightIcon?: ReactNode;
  /** Replaces the label with a spinner and blocks interaction. */
  loading?: boolean;
  /** Shows a success state after an async action. */
  success?: boolean;
  successText?: ReactNode;
  successIcon?: ReactNode;
  /** Persistent active/selected state. */
  active?: boolean;
  /** Draw the focus ring inside the button. */
  insetFocus?: boolean;
  /** Flat left / right corners (for joined / segmented buttons). */
  leftFlat?: boolean;
  rightFlat?: boolean;
  /** Remove horizontal padding (icon-only). */
  noSidePadding?: boolean;
  marginLeft?: boolean;
  marginRight?: boolean;
  /** Disable the press scale animation. */
  preventClickAnimation?: boolean;
  /** Blur the button after a mouse click (default `true`). */
  blurOnMouseUp?: boolean;
}

const Check = (
  <svg className="btn__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M20 6 9 17l-5-5" />
  </svg>
);

/**
 * Primary action control.
 *
 * Variants: `primary`, `secondary`, `ghost`, `danger`; sizes: `xs`–`xl`.
 * `color` recolours any variant (including secondary/ghost). Element states:
 * `floating`, `glass={false}`, `active`. Content: `leftIcon`/`rightIcon`,
 * `loading`, `success`/`successText`. Shape: `leftFlat`/`rightFlat`,
 * `noSidePadding`, `insetFocus`, `preventClickAnimation`, `blurOnMouseUp`.
 * All native button props (`aria-*`, `name`, `tabIndex`, `onClick`, …) forward.
 *
 * Accessibility: a real `<button>`; always provide a visible label or an
 * `aria-label`. `loading` sets `aria-busy` and disables the button.
 *
 * Tokens: `--rumahl-ui-color-*`, `--control-fill/-border/-inset/-filter`.
 */
export function Button({
  variant = "secondary",
  size = "md",
  className,
  type,
  title,
  floating,
  glass = true,
  color,
  backgroundColor,
  leftIcon,
  rightIcon,
  loading,
  success,
  successText,
  successIcon,
  active,
  insetFocus,
  leftFlat,
  rightFlat,
  noSidePadding,
  marginLeft,
  marginRight,
  preventClickAnimation,
  blurOnMouseUp = true,
  disabled,
  onClick,
  onMouseUp,
  style,
  children,
  ...props
}: ButtonProps) {
  const showSuccess = Boolean(success) && !loading;
  const content = loading ? (
    <>
      <span className="btn__loader" aria-hidden="true" />
      {successText ? null : children}
    </>
  ) : showSuccess ? (
    <>
      {successIcon ?? Check}
      {successText || children}
    </>
  ) : (
    <>
      {leftIcon ? <span className="btn__icon">{leftIcon}</span> : null}
      {children}
      {rightIcon ? <span className="btn__icon">{rightIcon}</span> : null}
    </>
  );
  const button = (
    <button
      type={type ?? "button"}
      className={buttonClass(variant, size, className)}
      data-floating={floating ? "" : undefined}
      data-glass={glass ? undefined : "false"}
      data-colored={color ? "true" : undefined}
      data-bg={backgroundColor ? "true" : undefined}
      data-loading={loading ? "true" : undefined}
      data-success={showSuccess ? "true" : undefined}
      data-active={active ? "true" : undefined}
      data-inset-focus={insetFocus ? "true" : undefined}
      data-left-flat={leftFlat ? "true" : undefined}
      data-right-flat={rightFlat ? "true" : undefined}
      data-no-side-padding={noSidePadding ? "true" : undefined}
      data-no-press={preventClickAnimation ? "true" : undefined}
      aria-busy={loading ? true : undefined}
      disabled={disabled || loading}
      onClick={onClick}
      onMouseUp={(event) => {
        onMouseUp?.(event);
        if (blurOnMouseUp && !event.defaultPrevented) event.currentTarget.blur();
      }}
      style={
        {
          ...colorStyle(color),
          ...(color ? { "--btn-color": color } : {}),
          ...(backgroundColor
            ? { "--btn-bg": backgroundColor, "--btn-bg-ink": onColor(backgroundColor) }
            : {}),
          ...(marginLeft ? { marginLeft: 8 } : {}),
          ...(marginRight ? { marginRight: 8 } : {}),
          ...style
        } as CSSProperties
      }
      {...props}
    >
      {content}
    </button>
  );
  return title ? <RumahlTooltip content={title}>{button}</RumahlTooltip> : button;
}

export interface ButtonLinkProps extends AnchorHTMLAttributes<HTMLAnchorElement>, ColorProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  leftFlat?: boolean;
  rightFlat?: boolean;
  children: ReactNode;
}

/**
 * A link styled exactly like {@link Button}, for navigation.
 *
 * Accessibility: renders an `<a>`; provide `href` and a label. Use `Button` for
 * actions so the element matches the intent.
 */
export function ButtonLink({
  variant = "secondary",
  size = "md",
  className,
  leftIcon,
  rightIcon,
  leftFlat,
  rightFlat,
  color,
  style,
  children,
  ...props
}: ButtonLinkProps) {
  return (
    <a
      className={buttonClass(variant, size, className)}
      data-colored={color ? "true" : undefined}
      data-left-flat={leftFlat ? "true" : undefined}
      data-right-flat={rightFlat ? "true" : undefined}
      style={
        {
          ...colorStyle(color),
          ...(color ? { "--btn-color": color } : {}),
          ...style
        } as CSSProperties
      }
      {...props}
    >
      {leftIcon ? <span className="btn__icon">{leftIcon}</span> : null}
      {children}
      {rightIcon ? <span className="btn__icon">{rightIcon}</span> : null}
    </a>
  );
}
