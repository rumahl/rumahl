import type { ComponentProps, ReactNode } from "react";

type InputProps = ComponentProps<"input">;

/** Native form semantics with the shared capsule material. */
export function RumahlInput({ className = "", ...props }: InputProps) {
  return <input {...props} className={`rumahl-input ${className}`} />;
}

export function RumahlTextArea({ className = "", ...props }: ComponentProps<"textarea">) {
  return <textarea {...props} className={`rumahl-textarea ${className}`} />;
}

type ChoiceProps = Omit<InputProps, "type" | "children"> & { label: ReactNode };
export function RumahlCheckbox({ label, className = "", ...props }: ChoiceProps) {
  return <label className="rumahl-choice"><input {...props} type="checkbox" className={`rumahl-checkbox ${className}`} /><span>{label}</span></label>;
}
export function RumahlRadio({ label, className = "", ...props }: ChoiceProps) {
  return <label className="rumahl-choice"><input {...props} type="radio" className={`rumahl-radio ${className}`} /><span>{label}</span></label>;
}

/** Input with an inset prefix and an optional trailing action or unit. */
export function RumahlInputGroup({ prefix, suffix, className = "", ...props }: Omit<InputProps, "prefix"> & { prefix?: ReactNode; suffix?: ReactNode }) {
  return <span className="rumahl-input-group">
    {prefix ? <span className="rumahl-input-group__affix">{prefix}</span> : null}
    <RumahlInput {...props} className={className} />
    {suffix ? <span className="rumahl-input-group__affix">{suffix}</span> : null}
  </span>;
}
