import type { ElementType, HTMLAttributes, KeyboardEvent, MouseEvent, ReactNode } from "react";

export interface ClickableProps extends HTMLAttributes<HTMLElement> {
  as?: ElementType;
  disabled?: boolean;
  /** Called after a pointer click or Enter/Space activation. */
  onActivate?: () => void;
  children?: ReactNode;
}

/**
 * Makes non-button content keyboard-accessible: adds `role="button"`,
 * `tabIndex` and Enter/Space handling. Prefer `Button`/`IconButton` for real
 * actions; use this to wrap composite content (a card, an avatar + name).
 */
export function Clickable({
  as: Tag = "span",
  disabled,
  onActivate,
  onClick,
  onKeyDown,
  className = "",
  role = "button",
  tabIndex,
  children,
  ...props
}: ClickableProps) {
  return (
    <Tag
      role={role}
      tabIndex={disabled ? -1 : tabIndex ?? 0}
      aria-disabled={disabled ? true : undefined}
      className={`rumahl-clickable ${className}`}
      onClick={(event: MouseEvent<HTMLElement>) => {
        if (disabled) {
          event.preventDefault();
          return;
        }
        onClick?.(event);
        onActivate?.();
      }}
      onKeyDown={(event: KeyboardEvent<HTMLElement>) => {
        if (!disabled && (event.key === "Enter" || event.key === " ")) {
          event.preventDefault();
          event.currentTarget.click();
        }
        onKeyDown?.(event);
      }}
      {...props}
    >
      {children}
    </Tag>
  );
}
