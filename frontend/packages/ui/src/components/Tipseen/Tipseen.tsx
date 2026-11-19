import type { ReactNode } from "react";
import { colorStyle, onColor } from "../../tint";

export interface TipseenProps {
  title: ReactNode;
  children: ReactNode;
  onClose?: () => void;
  closeLabel?: string;
  /** Optional action row (buttons). */
  actions?: ReactNode;
  /** Custom accent colour (default: theme accent). */
  color?: string | undefined;
  /** Explicit fill colour (overrides the accent fill). */
  backgroundColor?: string | undefined;
  className?: string;
}

/** Rich, dismissible hint card (coachmark). Accent-filled by default. */
export function Tipseen({ title, children, onClose, closeLabel = "Close", actions, color, backgroundColor, className = "" }: TipseenProps) {
  return (
    <div
      className={`rumahl-tipseen ${className}`}
      role="dialog"
      style={{
        ...colorStyle(color),
        ...(backgroundColor ? { background: backgroundColor, color: onColor(backgroundColor) } : {})
      }}
    >
      {onClose ? (
        <button type="button" className="rumahl-tipseen__close" aria-label={closeLabel} onClick={onClose}>
          ×
        </button>
      ) : null}
      <div className="rumahl-tipseen__title">{title}</div>
      <div className="rumahl-tipseen__body">{children}</div>
      {actions ? <div className="rumahl-tipseen__actions">{actions}</div> : null}
    </div>
  );
}
