import { useEffect, useId, useRef, type CSSProperties, type ReactNode } from "react";
import { Button } from "../Button/Button";
import { RumahlInputGroup, type ValidationProps } from "../RumahlInputs/RumahlInputs";
import type { ControlSize } from "../../size";
import { colorStyle, onColor } from "../../tint";

/** Search field with a clear button (an {@link RumahlInputGroup}). */
export function RumahlSearch({
  label,
  clearLabel,
  value,
  onChange,
  disabled = false,
  status,
  size,
}: ValidationProps & {
  label: string;
  clearLabel: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  size?: ControlSize | undefined;
}) {
  const ref = useRef<HTMLInputElement>(null);
  return (
    <RumahlInputGroup
      {...(status ? { status } : {})}
      {...(size ? { size } : {})}
      ref={ref}
      type="search"
      aria-label={label}
      placeholder={label}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
      suffix={
        value ? (
          <Button
            size={size ?? "sm"}
            disabled={disabled}
            aria-label={clearLabel}
            onClick={() => {
              onChange("");
              ref.current?.focus();
            }}
          >
            ×
          </Button>
        ) : undefined
      }
    />
  );
}

/** Numeric stepper with decrement/increment buttons and bounds. */
export function RumahlStepper({
  label,
  decreaseLabel,
  increaseLabel,
  value,
  onChange,
  min = 0,
  max = 100,
  step = 1,
  disabled = false,
  status,
  size,
}: ValidationProps & {
  label: string;
  decreaseLabel: string;
  increaseLabel: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  size?: ControlSize | undefined;
}) {
  const clamp = (next: number) =>
    Math.min(max, Math.max(min, Number(next.toFixed(10))));
  return (
    <RumahlInputGroup
      {...(status ? { status } : {})}
      {...(size ? { size } : {})}
      type="number"
      aria-label={label}
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onChange={(event) => {
        const next = event.target.valueAsNumber;
        if (Number.isFinite(next)) onChange(clamp(next));
      }}
      prefix={
        <Button
          size={size ?? "sm"}
          aria-label={decreaseLabel}
          disabled={disabled || value <= min}
          onClick={() => onChange(clamp(value - step))}
        >
          −
        </Button>
      }
      suffix={
        <Button
          size={size ?? "sm"}
          aria-label={increaseLabel}
          disabled={disabled || value >= max}
          onClick={() => onChange(clamp(value + step))}
        >
          +
        </Button>
      }
    />
  );
}

/** Small status badge with a tone (`neutral`/`success`/`warning`/`danger`). */
export function RumahlBadge({
  children,
  tone = "neutral",
  size,
  color,
  backgroundColor,
  style,
}: {
  children: ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger";
  size?: ControlSize;
  color?: string | undefined;
  backgroundColor?: string | undefined;
  style?: CSSProperties;
}) {
  return (
    <span
      className={`rumahl-badge rumahl-badge--${tone}`}
      data-size={size}
      style={{
        ...colorStyle(color),
        ...(color ? { "--badge-color": color } : {}),
        ...(backgroundColor ? { background: backgroundColor, color: onColor(backgroundColor), borderColor: "transparent" } : {}),
        ...style
      }}
    >
      {children}
    </span>
  );
}
/** Inline notice/callout with a title, body and optional action. */
export function RumahlNotice({
  title,
  children,
  tone = "info",
  action,
  size,
  color,
  backgroundColor,
  style,
}: {
  title: string;
  children: ReactNode;
  tone?: "info" | "warning" | "danger";
  action?: ReactNode;
  size?: ControlSize;
  color?: string | undefined;
  backgroundColor?: string | undefined;
  style?: CSSProperties;
}) {
  return (
    <div
      className={`rumahl-notice rumahl-notice--${tone}`}
      data-size={size}
      style={{
        ...colorStyle(color),
        ...(backgroundColor ? { background: backgroundColor, color: onColor(backgroundColor), borderColor: "transparent" } : {}),
        ...style
      }}
    >
      <span className="rumahl-notice__symbol" aria-hidden="true">
        {tone === "info" ? "i" : "!"}
      </span>
      <div>
        <strong>{title}</strong>
        <p>{children}</p>
      </div>
      {action}
    </div>
  );
}

/** Tabs with a shared moving indicator (a rumahl toggle group). */
export function RumahlTabs({
  label,
  tabs,
  value,
  onChange,
  size,
  color,
  style,
}: {
  label: string;
  tabs: readonly {
    id: string;
    label: string;
    content: ReactNode;
    disabled?: boolean;
  }[];
  value: string;
  onChange: (value: string) => void;
  size?: ControlSize;
  color?: string | undefined;
  style?: CSSProperties;
}) {
  const id = useId();
  const groupRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const group = groupRef.current;
    if (!group) return;
    const measure = () => {
      const selected = group.querySelector<HTMLButtonElement>('[role="tab"][aria-selected="true"]');
      group.dataset.hasSelection = selected ? "true" : "false";
      if (!selected) return;
      group.style.setProperty("--selection-left", `${selected.offsetLeft}px`);
      group.style.setProperty("--selection-width", `${selected.offsetWidth}px`);
    };
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(group);
    group.querySelectorAll('[role="tab"]').forEach((tab) => observer?.observe(tab));
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [tabs, value]);
  return (
    <div className="rumahl-tabs" data-size={size} style={{ ...colorStyle(color), ...style }}>
      <div
        ref={groupRef}
        className="rumahl-button-group"
        role="tablist"
        aria-label={label}
        onKeyDown={(event) => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key))
            return;
          const enabled = tabs.filter((tab) => !tab.disabled);
          const current = enabled.findIndex(
            (tab) => `${id}-${tab.id}` === (event.target as HTMLElement).id,
          );
          if (current < 0) return;
          event.preventDefault();
          const next =
            enabled[
              event.key === "Home"
                ? 0
                : event.key === "End"
                  ? enabled.length - 1
                  : (current +
                      (event.key === "ArrowRight" ? 1 : -1) +
                      enabled.length) %
                    enabled.length
            ];
          if (next) {
            onChange(next.id);
            document.getElementById(`${id}-${next.id}`)?.focus();
          }
        }}
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={`${id}-${tab.id}`}
            aria-controls={`${id}-${tab.id}-panel`}
            aria-selected={value === tab.id}
            disabled={tab.disabled}
            tabIndex={value === tab.id ? 0 : -1}
            onClick={() => onChange(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {tabs.map((tab) => (
        <div
          key={tab.id}
          role="tabpanel"
          id={`${id}-${tab.id}-panel`}
          aria-labelledby={`${id}-${tab.id}`}
          hidden={value !== tab.id}
          tabIndex={0}
        >
          {tab.content}
        </div>
      ))}
    </div>
  );
}
