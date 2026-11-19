import type { ControlSize } from "../../size";
import { colorStyle } from "../../tint";

export interface CounterProps {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  /** Accessible group name, e.g. "Quantity". */
  label?: string;
  size?: ControlSize | undefined;
  /** Custom accent colour. */
  color?: string | undefined;
  className?: string;
}

/** Compact bounded number field with − / + buttons. */
export function Counter({
  value,
  onChange,
  min = Number.NEGATIVE_INFINITY,
  max = Number.POSITIVE_INFINITY,
  step = 1,
  disabled,
  label,
  size,
  color,
  className = ""
}: CounterProps) {
  return (
    <div className={`rumahl-counter ${className}`} data-size={size} role="group" aria-label={label} style={colorStyle(color)}>
      <button
        type="button"
        className="rumahl-counter__button"
        aria-label={label ? `Decrease ${label}` : "Decrease"}
        disabled={disabled || value <= min}
        onClick={() => onChange(Math.max(min, value - step))}
      >
        −
      </button>
      <span className="rumahl-counter__value" aria-live="polite">
        {value}
      </span>
      <button
        type="button"
        className="rumahl-counter__button"
        aria-label={label ? `Increase ${label}` : "Increase"}
        disabled={disabled || value >= max}
        onClick={() => onChange(Math.min(max, value + step))}
      >
        +
      </button>
    </div>
  );
}
