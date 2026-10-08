import type { KeyboardEvent, ReactNode } from "react";

export interface GroupOption<T extends string> { value: T; label: string; icon?: ReactNode; disabled?: boolean }
type Props<T extends string> = {
  label: string;
  options: readonly GroupOption<T>[];
  disabled?: boolean;
  iconOnly?: boolean;
} & ({ multiple?: false; value: T; onChange: (value: T) => void } | { multiple: true; value: readonly T[]; onChange: (value: T[]) => void });

/** Segmented single selection or independent toggle buttons, with native button semantics. */
export function RumahlButtonGroup<T extends string>(props: Props<T>) {
  function keyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
    const index = buttons.indexOf(event.target as HTMLButtonElement);
    if (index < 0 || !buttons.length) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
  return <div className="rumahl-button-group" role="group" aria-label={props.label} onKeyDown={keyboard}>
    {props.options.map(option => <button key={option.value} type="button" disabled={props.disabled || option.disabled}
      aria-label={props.iconOnly ? option.label : undefined} title={props.iconOnly ? option.label : undefined}
      aria-pressed={props.multiple ? props.value.includes(option.value) : props.value === option.value}
      onClick={() => {
        if (props.multiple) props.onChange(props.value.includes(option.value) ? props.value.filter(value => value !== option.value) : [...props.value, option.value]);
        else props.onChange(option.value);
      }}>
      {option.icon ? <span aria-hidden="true">{option.icon}</span> : null}{props.iconOnly ? null : option.label}
    </button>)}
  </div>;
}
