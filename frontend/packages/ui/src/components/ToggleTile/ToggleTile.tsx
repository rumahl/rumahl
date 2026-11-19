import type { ButtonHTMLAttributes, ReactNode } from "react";
import { colorStyle, type ColorProps } from "../../tint";

export interface ToggleTileProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  label: ReactNode;
  icon?: ReactNode;
  description?: ReactNode;
  active?: boolean | undefined;
}

/** Quick-settings toggle tile (control centre). */
export function ToggleTile({
  label,
  icon,
  description,
  active,
  className = "",
  type = "button",
  color,
  style,
  ...props
}: ToggleTileProps) {
  return (
    <button
      type={type}
      aria-pressed={active}
      data-active={active ? "true" : undefined}
      className={`rumahl-toggle-tile ${className}`}
      style={{ ...colorStyle(color), ...style }}
      {...props}
    >
      {icon ? (
        <span className="rumahl-toggle-tile__icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span className="rumahl-toggle-tile__label">{label}</span>
      {description ? <span className="rumahl-toggle-tile__description">{description}</span> : null}
    </button>
  );
}
