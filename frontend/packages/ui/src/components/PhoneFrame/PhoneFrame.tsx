import type { CSSProperties, ReactNode } from "react";

export interface PhoneFrameProps {
  children: ReactNode;
  /** Status bar content (time + status icons). */
  status?: ReactNode;
  /** Bottom bar (tab bar / home). */
  footer?: ReactNode;
  width?: number;
  height?: number;
  className?: string;
  style?: CSSProperties;
}

/**
 * Presentational phone surface: a rounded frame with a status bar slot, a
 * scrollable content area, an optional bottom bar and a home indicator.
 *
 * It is a **building block** for mobile/hybrid layouts (and a future companion
 * app). It carries no OS behaviour — the launcher's logic (app catalog, opening
 * apps, folders) stays in the shell.
 */
export function PhoneFrame({
  children,
  status,
  footer,
  width = 340,
  height = 640,
  className = "",
  style
}: PhoneFrameProps) {
  return (
    <div className={`rumahl-phone ${className}`} style={{ width, height, ...style }}>
      {status ? <div className="rumahl-phone__status">{status}</div> : null}
      <div className="rumahl-phone__content">{children}</div>
      {footer ? <div className="rumahl-phone__footer">{footer}</div> : null}
      <span className="rumahl-phone__home" aria-hidden="true" />
    </div>
  );
}
