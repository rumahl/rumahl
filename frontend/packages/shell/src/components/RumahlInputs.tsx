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
