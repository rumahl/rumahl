import { useEffect, useId, useRef, useState, type CSSProperties, type ComponentProps, type ReactNode } from "react";
import type { SizeProps } from "../../size";
import { colorStyle, type ColorProps } from "../../tint";

export type ValidationStatus = "neutral" | "error" | "success";
export interface ValidationProps {
  status?: ValidationStatus;
}
export interface InputFeedbackProps {
  error?: string;
  /** Shows a spinner in the field while an async value resolves. */
  loading?: boolean | undefined;
}
type InputProps = Omit<ComponentProps<"input">, "size"> & ValidationProps;

export function validationAttributes(status?: ValidationStatus) {
  return {
    "data-status": status,
    "aria-invalid": status === "error" ? true : undefined,
  };
}

/** Custom feedback keeps the message associated with the input, beyond color. */
export function RumahlInputFeedback({
  error,
  id,
  loading,
  children,
}: {
  error?: string | undefined;
  id: string;
  loading?: boolean | undefined;
  children: ReactNode;
}) {
  if (error === undefined && !loading) return children;
  return (
    <span className="rumahl-input-feedback">
      <span className="rumahl-input-feedback__control">
        {children}
        {loading ? (
          <span className="rumahl-input-feedback__icon rumahl-input-feedback__icon--loading" aria-hidden="true" />
        ) : error ? (
          <span className="rumahl-input-feedback__icon" aria-hidden="true">
            !
          </span>
        ) : null}
      </span>
      {error ? (
        <span id={id} className="rumahl-input-feedback__message" role="alert">
          {error}
        </span>
      ) : null}
    </span>
  );
}

/**
 * Single-line text field with validation feedback.
 *
 * Props: native `<input>` props plus `status` (`neutral`/`error`/`success`),
 * `error` (message + icon), `floating` (over a background) and `glass={false}`
 * (flat). `className`/`style` are forwarded.
 *
 * Accessibility: associates its error message via `aria-describedby` and sets
 * `aria-invalid`; always provide a label or `aria-label`.
 *
 * Tokens: `--control-fill/-border/-inset/-filter`, `--rumahl-ui-color-*`.
 */
export function RumahlInput({
  className = "",
  status,
  error,
  material,
  floating,
  glass = true,
  size,
  color,
  backgroundColor,
  style,
  loading,
  ...props
}: InputProps &
  InputFeedbackProps &
  SizeProps &
  ColorProps & {
    material?: "glass";
    floating?: boolean;
    /** Override: `glass={false}` renders a flat field without glass. */
    glass?: boolean;
    /** Explicit fill colour (independent of the accent `color`). */
    backgroundColor?: string | undefined;
  }) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id} loading={loading}>
      <input
        {...validationAttributes(status)}
        {...props}
        aria-busy={loading ? true : undefined}
        aria-invalid={
          error
            ? true
            : (props["aria-invalid"] ?? (status === "error" ? true : undefined))
        }
        data-status={error ? "error" : status}
        aria-describedby={
          [props["aria-describedby"], error ? id : undefined]
            .filter(Boolean)
            .join(" ") || undefined
        }
        data-material={material}
        data-floating={floating ? "" : undefined}
        data-glass={glass ? undefined : "false"}
        data-size={size}
        data-tinted={color ? "true" : undefined}
        data-bg={backgroundColor ? "true" : undefined}
        style={{ ...colorStyle(color), ...(backgroundColor ? { "--field-bg": backgroundColor } : {}), ...style }}
        className={`rumahl-input ${className}`}
      />
    </RumahlInputFeedback>
  );
}

/** Multi-line text field. Same props and validation as {@link RumahlInput}. */
export function RumahlTextArea({
  className = "",
  status,
  error,
  size,
  color,
  backgroundColor,
  style,
  loading,
  ...props
}: ComponentProps<"textarea"> & ValidationProps & InputFeedbackProps & SizeProps & ColorProps & { backgroundColor?: string | undefined }) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id} loading={loading}>
      <textarea
        {...validationAttributes(status)}
        {...props}
        aria-busy={loading ? true : undefined}
        data-size={size}
        data-tinted={color ? "true" : undefined}
        data-bg={backgroundColor ? "true" : undefined}
        style={{ ...colorStyle(color), ...(backgroundColor ? { "--field-bg": backgroundColor } : {}), ...style }}
        aria-invalid={
          error
            ? true
            : (props["aria-invalid"] ?? (status === "error" ? true : undefined))
        }
        data-status={error ? "error" : status}
        aria-describedby={
          [props["aria-describedby"], error ? id : undefined]
            .filter(Boolean)
            .join(" ") || undefined
        }
        className={`rumahl-textarea ${className}`}
      />
    </RumahlInputFeedback>
  );
}

type ChoiceProps = Omit<InputProps, "type" | "children"> & SizeProps & ColorProps & { label: ReactNode };
/** Labelled checkbox (native input; keyboard and form friendly). */
export function RumahlCheckbox({
  label,
  className = "",
  status,
  size,
  color,
  style,
  ...props
}: ChoiceProps) {
  return (
    <label className="rumahl-choice" data-size={size} style={{ ...colorStyle(color), ...style }}>
      <input
        {...validationAttributes(status)}
        {...props}
        type="checkbox"
        className={`rumahl-checkbox ${className}`}
      />
      <span>{label}</span>
    </label>
  );
}
/** Labelled radio button. Group several with the same `name`. */
export function RumahlRadio({
  label,
  className = "",
  status,
  size,
  color,
  style,
  ...props
}: ChoiceProps) {
  return (
    <label className="rumahl-choice" data-size={size} style={{ ...colorStyle(color), ...style }}>
      <input
        {...validationAttributes(status)}
        {...props}
        type="radio"
        className={`rumahl-radio ${className}`}
      />
      <span>{label}</span>
    </label>
  );
}

/** Input with an inset prefix and an optional trailing action or unit. */
/** Input with an inset prefix/suffix (icon, unit or action). */
export function RumahlInputGroup({
  prefix,
  suffix,
  className = "",
  status,
  error,
  floating,
  glass = true,
  size,
  color,
  backgroundColor,
  style,
  ...props
}: Omit<InputProps, "prefix"> &
  InputFeedbackProps &
  SizeProps &
  ColorProps & {
    prefix?: ReactNode;
    suffix?: ReactNode;
    floating?: boolean;
    /** Override: `glass={false}` renders a flat field without glass. */
    glass?: boolean;
    /** Explicit fill colour (independent of the accent `color`). */
    backgroundColor?: string | undefined;
  }) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id}>
      <span
        className="rumahl-input-group"
        data-status={error ? "error" : status}
        data-floating={floating ? "" : undefined}
        data-glass={glass ? undefined : "false"}
        data-size={size}
        style={{ ...colorStyle(color), ...style }}
      >
        {prefix ? (
          <span className="rumahl-input-group__affix">{prefix}</span>
        ) : null}
        <RumahlInput
          {...props}
          {...(status ? { status } : {})}
          {...(color ? { color } : {})}
          {...(backgroundColor ? { backgroundColor } : {})}
          aria-invalid={
            error
              ? true
              : (props["aria-invalid"] ??
                (status === "error" ? true : undefined))
          }
          aria-describedby={
            [props["aria-describedby"], error ? id : undefined]
              .filter(Boolean)
              .join(" ") || undefined
          }
          className={className}
        />
        {suffix ? (
          <span className="rumahl-input-group__affix">{suffix}</span>
        ) : null}
      </span>
    </RumahlInputFeedback>
  );
}

/** Related settings with native legend and disabled-fieldset behavior. */
/** Related settings in a native `<fieldset>` with a legend and description. */
export function RumahlFieldGroup({
  label,
  description,
  status,
  children,
  className = "",
  size,
  ...props
}: ComponentProps<"fieldset"> &
  ValidationProps & SizeProps & { label: ReactNode; description?: ReactNode }) {
  const id = useId();
  return (
    <fieldset
      {...validationAttributes(status)}
      {...props}
      className={`rumahl-field-group ${className}`}
      data-size={size}
      aria-describedby={
        [props["aria-describedby"], description ? id : undefined]
          .filter(Boolean)
          .join(" ") || undefined
      }
    >
      <legend>{label}</legend>
      <div className="rumahl-field-group__fields">{children}</div>
      {description ? (
        <small id={id} role="status">
          {description}
        </small>
      ) : null}
    </fieldset>
  );
}

/** Native checkboxes support keyboard selection and repeated values in FormData. */
/** A group of checkboxes for selecting several values. */
export function RumahlMultiSelect({
  label,
  options,
  value,
  onChange,
  name,
  disabled,
  status,
  description,
  size,
}: ValidationProps &
  SizeProps & {
    label: string;
    options: readonly { value: string; label: string; disabled?: boolean }[];
    value: readonly string[];
    onChange: (value: string[]) => void;
    name?: string;
    disabled?: boolean;
    description?: ReactNode;
  }) {
  return (
    <RumahlFieldGroup
      label={label}
      disabled={disabled}
      {...(status ? { status } : {})}
      {...(size ? { size } : {})}
      description={description}
    >
      {options.map((option) => (
        <RumahlCheckbox
          key={option.value}
          name={name}
          value={option.value}
          label={option.label}
          checked={value.includes(option.value)}
          disabled={option.disabled}
          onChange={(event) =>
            onChange(
              event.target.checked
                ? [...value, option.value]
                : value.filter((item) => item !== option.value),
            )
          }
        />
      ))}
    </RumahlFieldGroup>
  );
}

/** Thumb travel in px, read from the size-driven `--switch-travel` token. */
function switchTravel(element: HTMLElement): number {
  const value = Number.parseFloat(getComputedStyle(element).getPropertyValue("--switch-travel"));
  return Number.isFinite(value) && value > 0 ? value : 18;
}

/** Toggle switch (`role="switch"`); click or drag to change. */
export function RumahlSwitch({
  label,
  status,
  className = "",
  size,
  color,
  style,
  ...props
}: ChoiceProps) {
  const drag = useRef<{ id: number; start: number; origin: number; position: number; travel: number; moved: boolean } | null>(null);
  const suppressClick = useRef(false);
  const ignoreChange = useRef(false);
  const [position, setPosition] = useState<number | null>(null);
  const clearDrag = () => {
    drag.current = null;
    setPosition(null);
  };
  return (
    <label className="appearance-switch rumahl-switch" data-status={status} data-size={size} style={{ ...colorStyle(color), ...style }}>
      <span>{label}</span>
      <input
        {...validationAttributes(status)}
        {...props}
        type="checkbox"
        role="switch"
        className={className}
        data-dragging={position !== null ? "true" : undefined}
        style={{ ...(position !== null ? { "--switch-position": `${position}px` } : {}) } as CSSProperties}
        onPointerDown={(event) => {
          props.onPointerDown?.(event);
          suppressClick.current = false;
          ignoreChange.current = false;
          if (event.defaultPrevented || event.button !== 0 || event.isPrimary === false || event.currentTarget.matches(":disabled")) return;
          const travel = switchTravel(event.currentTarget);
          const origin = event.currentTarget.checked ? travel : 0;
          drag.current = { id: event.pointerId, start: event.clientX, origin, position: origin, travel, moved: false };
          event.currentTarget.setPointerCapture?.(event.pointerId);
          event.currentTarget.focus();
        }}
        onPointerMove={(event) => {
          props.onPointerMove?.(event);
          const current = drag.current;
          if (!current || current.id !== event.pointerId || event.defaultPrevented) return;
          const distance = event.clientX - current.start;
          current.moved ||= Math.abs(distance) > 3;
          if (!current.moved) return;
          current.position = Math.max(0, Math.min(current.travel, current.origin + distance));
          setPosition(current.position);
        }}
        onPointerUp={(event) => {
          props.onPointerUp?.(event);
          const current = drag.current;
          if (!current || current.id !== event.pointerId) return;
          const input = event.currentTarget;
          clearDrag();
          if (input.hasPointerCapture?.(event.pointerId)) input.releasePointerCapture(event.pointerId);
          if (!current.moved) return;
          if (event.defaultPrevented || input.matches(":disabled") || input.checked === (current.position >= current.travel / 2)) {
            suppressClick.current = true;
            return;
          }
          // Commit the change through a real click so React's own checkbox
          // onChange path runs (controlled state, form semantics, jsdom alike).
          input.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
          suppressClick.current = true;
        }}
        onPointerCancel={(event) => {
          props.onPointerCancel?.(event);
          clearDrag();
        }}
        onLostPointerCapture={(event) => {
          props.onLostPointerCapture?.(event);
          clearDrag();
        }}
        onClick={(event) => {
          // Suppress only the physical click following a drag, never keyboard activation.
          if (suppressClick.current && event.detail !== 0) {
            suppressClick.current = false;
            ignoreChange.current = true;
            event.stopPropagation();
            return;
          }
          ignoreChange.current = false;
          props.onClick?.(event);
        }}
        onChange={(event) => {
          if (ignoreChange.current) {
            ignoreChange.current = false;
            // The native toggle already happened; snap a controlled switch back
            // to the value React owns.
            if (props.checked !== undefined) {
              (event.target as HTMLInputElement).checked = props.checked;
            }
            return;
          }
          props.onChange?.(event);
        }}
      />
    </label>
  );
}

/** Labelled range slider. */
export function RumahlRange({
  label,
  status,
  className = "",
  size,
  ...props
}: Omit<InputProps, "type"> & SizeProps & { label: string }) {
  const minimum = Number(props.min ?? 0);
  const maximum = Number(props.max ?? 100);
  const [value, setValue] = useState(Number(props.defaultValue ?? (minimum + maximum) / 2));
  const track = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const input = track.current?.querySelector("input");
    if (!input) return;
    setValue(input.valueAsNumber);
    const reset = () => queueMicrotask(() => setValue(input.valueAsNumber));
    input.form?.addEventListener("reset", reset);
    return () => input.form?.removeEventListener("reset", reset);
  }, [props.min, props.max, props.step]);
  const current = Number(props.value ?? value);
  const percent = maximum > minimum ? Math.max(0, Math.min(100, (current - minimum) / (maximum - minimum) * 100)) : 0;
  return (
    <label className="rumahl-range" data-status={status} data-size={size}>
      <span>{label}</span>
      <span ref={track} className="rumahl-range__slider" style={{ "--range-position": `${percent}%`, "--range-offset": `${percent * 0.18}px` } as CSSProperties}>
        <span className="rumahl-range__channel" aria-hidden="true"><span /></span>
        <span className="rumahl-range__thumb" aria-hidden="true" />
        <input
          {...validationAttributes(status)}
          {...props}
          type="range"
          className={className}
          onChange={event => {
            setValue(event.target.valueAsNumber);
            props.onChange?.(event);
          }}
        />
      </span>
    </label>
  );
}

/** Labelled file input. */
export function RumahlFileInput({
  label,
  status,
  className = "",
  size,
  ...props
}: Omit<InputProps, "type"> & SizeProps & { label: string }) {
  return (
    <label className="rumahl-file" data-status={status} data-size={size}>
      <span>{label}</span>
      <input
        {...validationAttributes(status)}
        {...props}
        type="file"
        className={className}
      />
    </label>
  );
}
