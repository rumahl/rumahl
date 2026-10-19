import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";
import { colorStyle, type ColorProps } from "../../tint";

/** Status-item cluster (system tray / menu-bar right island). */
export function Tray({
  className = "",
  floating,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & { floating?: boolean | undefined }) {
  return (
    <div role="toolbar" className={`rumahl-tray ${className}`} data-floating={floating ? "" : undefined} {...props}>
      {children}
    </div>
  );
}

export interface TrayItemProps extends ButtonHTMLAttributes<HTMLButtonElement>, ColorProps {
  /** Accessible name. */
  label: string;
  active?: boolean;
  children: ReactNode;
}

/** A single tray icon button. */
export function TrayItem({ label, active, className = "", type = "button", color, style, children, ...props }: TrayItemProps) {
  return (
    <button
      type={type}
      aria-label={label}
      aria-pressed={active}
      data-active={active ? "true" : undefined}
      className={`rumahl-tray__item ${className}`}
      style={{ ...colorStyle(color), ...style }}
      {...props}
    >
      {children}
    </button>
  );
}
