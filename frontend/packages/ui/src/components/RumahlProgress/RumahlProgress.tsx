import type { CSSProperties } from "react";
import type { ControlSize } from "../../size";
import { colorStyle } from "../../tint";

/** Shared loading and percentage bar; the empty track is always visible. */
/** Determinate or indeterminate progress bar with a label. */
export function RumahlProgress({ value, label, size, color, style }: { value?: number; label: string; size?: ControlSize; color?: string | undefined; style?: CSSProperties }) {
  const percent = value === undefined || !Number.isFinite(value) ? undefined : Math.max(0, Math.min(100, value));
  return <div className="rumahl-progress" data-size={size} style={{ ...colorStyle(color), ...style }}>
    <span>{label}</span>
    <div className="rumahl-progress__track" role="progressbar" aria-label={label}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}
      data-indeterminate={percent === undefined ? "true" : undefined}
      style={{ "--progress-value": `${percent ?? 35}%` } as CSSProperties}>
      <span className="rumahl-progress__fill" />
    </div>
    {percent !== undefined ? <output>{Math.round(percent)}%</output> : null}
  </div>;
}

/** The stationary ring remains visible underneath the moving arc. */
export function RumahlSpinner({ label, size = "md", color, style }: { label: string; size?: ControlSize | undefined; color?: string | undefined; style?: CSSProperties }) {
  return <span className="rumahl-spinner" data-size={size} role="status" aria-label={label} style={{ ...colorStyle(color), ...style }}>
    <svg className="rumahl-spinner__ring" viewBox="0 0 40 40" aria-hidden="true">
      <circle className="rumahl-spinner__edge" cx="20" cy="20" r="16" />
      <circle className="rumahl-spinner__track" cx="20" cy="20" r="16" />
      <circle className="rumahl-spinner__arc" cx="20" cy="20" r="16" pathLength="100" />
    </svg>
  </span>;
}
