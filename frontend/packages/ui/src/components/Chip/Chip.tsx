import type { ButtonHTMLAttributes, ReactNode } from "react";
import { colorStyle, type ColorProps } from "../../tint";

export interface ChipProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  /** Selectable chip: marks pressed state. */
  selected?: boolean;
  /** Renders a remove button when provided. */
  onRemove?: () => void;
  removeLabel?: string;
  icon?: ReactNode;
  children: ReactNode;
}

/**
 * Compact pill for tags, filters or tokens. Pass `onClick` to make it
 * selectable, `onRemove` to add a remove affordance.
 */
export function Chip({
  selected,
  onRemove,
  removeLabel = "Remove",
  icon,
  onClick,
  disabled,
  className = "",
  color,
  style,
  children,
  ...props
}: ChipProps) {
  const interactive = Boolean(onClick);
  return (
    <span className={`rumahl-chip ${className}`} data-selected={selected ? "true" : undefined} style={{ ...colorStyle(color), ...style }}>
      {interactive ? (
        <button
          type="button"
          className="rumahl-chip__label"
          onClick={onClick}
          disabled={disabled}
          aria-pressed={selected}
          {...props}
        >
          {icon ? <span aria-hidden="true">{icon}</span> : null}
          {children}
        </button>
      ) : (
        <span className="rumahl-chip__label">
          {icon ? <span aria-hidden="true">{icon}</span> : null}
          {children}
        </span>
      )}
      {onRemove ? (
        <button
          type="button"
          className="rumahl-chip__remove"
          aria-label={removeLabel}
          onClick={onRemove}
          disabled={disabled}
        >
          ×
        </button>
      ) : null}
    </span>
  );
}
