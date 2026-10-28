import type { CSSProperties, ReactNode } from "react";
import { colorStyle } from "../../tint";

export interface Step {
  id: string;
  label: ReactNode;
}

export interface StepsProps {
  steps: readonly Step[];
  /** Id of the current step. */
  active: string;
  orientation?: "horizontal" | "vertical";
  /** Custom accent colour for the connectors and current marker. */
  color?: string | undefined;
  style?: CSSProperties;
  className?: string;
}

/** Multi-step progress indicator with done/current/upcoming states. */
export function Steps({ steps, active, orientation = "horizontal", color, style, className = "" }: StepsProps) {
  const index = steps.findIndex((step) => step.id === active);
  return (
    <ol className={`rumahl-steps ${className}`} data-orientation={orientation} style={{ ...colorStyle(color), ...style }}>
      {steps.map((step, position) => {
        const status = position < index ? "done" : position === index ? "active" : "todo";
        return (
          <li key={step.id} className="rumahl-step" data-status={status}>
            <span className="rumahl-step__marker" aria-hidden="true">
              {status === "done" ? "✓" : position + 1}
            </span>
            <span className="rumahl-step__label">{step.label}</span>
          </li>
        );
      })}
    </ol>
  );
}
