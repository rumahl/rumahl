import { useId, type ComponentProps, type ReactNode } from "react";

export type ValidationStatus = "neutral" | "error" | "success";
export interface ValidationProps {
  status?: ValidationStatus;
}
export interface InputFeedbackProps {
  error?: string;
}
type InputProps = ComponentProps<"input"> & ValidationProps;

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
  children,
}: {
  error?: string | undefined;
  id: string;
  children: ReactNode;
}) {
  if (error === undefined) return children;
  return (
    <span className="rumahl-input-feedback">
      <span className="rumahl-input-feedback__control">
        {children}
        {error ? (
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

export function RumahlInput({
  className = "",
  status,
  error,
  ...props
}: InputProps & InputFeedbackProps) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id}>
      <input
        {...validationAttributes(status)}
        {...props}
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
        className={`rumahl-input ${className}`}
      />
    </RumahlInputFeedback>
  );
}

export function RumahlTextArea({
  className = "",
  status,
  error,
  ...props
}: ComponentProps<"textarea"> & ValidationProps & InputFeedbackProps) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id}>
      <textarea
        {...validationAttributes(status)}
        {...props}
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

type ChoiceProps = Omit<InputProps, "type" | "children"> & { label: ReactNode };
export function RumahlCheckbox({
  label,
  className = "",
  status,
  ...props
}: ChoiceProps) {
  return (
    <label className="rumahl-choice">
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
export function RumahlRadio({
  label,
  className = "",
  status,
  ...props
}: ChoiceProps) {
  return (
    <label className="rumahl-choice">
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
export function RumahlInputGroup({
  prefix,
  suffix,
  className = "",
  status,
  error,
  ...props
}: Omit<InputProps, "prefix"> &
  InputFeedbackProps & { prefix?: ReactNode; suffix?: ReactNode }) {
  const id = useId();
  return (
    <RumahlInputFeedback error={error} id={id}>
      <span
        className="rumahl-input-group"
        data-status={error ? "error" : status}
      >
        {prefix ? (
          <span className="rumahl-input-group__affix">{prefix}</span>
        ) : null}
        <RumahlInput
          {...props}
          {...(status ? { status } : {})}
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
export function RumahlFieldGroup({
  label,
  description,
  status,
  children,
  className = "",
  ...props
}: ComponentProps<"fieldset"> &
  ValidationProps & { label: ReactNode; description?: ReactNode }) {
  const id = useId();
  return (
    <fieldset
      {...validationAttributes(status)}
      {...props}
      className={`rumahl-field-group ${className}`}
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
export function RumahlMultiSelect({
  label,
  options,
  value,
  onChange,
  name,
  disabled,
  status,
  description,
}: ValidationProps & {
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

export function RumahlSwitch({
  label,
  status,
  className = "",
  ...props
}: ChoiceProps) {
  return (
    <label className="appearance-switch rumahl-switch" data-status={status}>
      <span>{label}</span>
      <input
        {...validationAttributes(status)}
        {...props}
        type="checkbox"
        role="switch"
        className={className}
      />
    </label>
  );
}

export function RumahlRange({
  label,
  status,
  className = "",
  ...props
}: Omit<InputProps, "type"> & { label: string }) {
  return (
    <label className="rumahl-range" data-status={status}>
      <span>{label}</span>
      <input
        {...validationAttributes(status)}
        {...props}
        type="range"
        className={className}
      />
    </label>
  );
}

export function RumahlFileInput({
  label,
  status,
  className = "",
  ...props
}: Omit<InputProps, "type"> & { label: string }) {
  return (
    <label className="rumahl-file" data-status={status}>
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
