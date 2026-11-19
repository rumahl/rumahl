import type { ElementType, HTMLAttributes, ReactNode } from "react";

export interface LabelProps extends HTMLAttributes<HTMLLabelElement> {
  as?: ElementType;
  /** Renders for the linked control. */
  htmlFor?: string;
  /** Adds a required marker (also conveyed via `aria-required` on the field). */
  required?: boolean;
  /** Right-aligned hint, e.g. `Optional`. */
  optionalText?: string;
  tone?: "default" | "muted";
  children?: ReactNode;
}

/**
 * Field label. `required` adds a visual asterisk (mark the field with
 * `required`/`aria-required` too so it is announced); `optionalText` adds a
 * muted hint.
 */
export function Label({
  as: Tag = "label",
  required,
  optionalText,
  tone = "default",
  className = "",
  children,
  ...props
}: LabelProps) {
  return (
    <Tag
      data-tone={tone === "default" ? undefined : tone}
      className={`rumahl-label ${className}`}
      {...props}
    >
      {children}
      {required ? (
        <span className="rumahl-label__required" aria-hidden="true">
          *
        </span>
      ) : null}
      {optionalText ? <span className="rumahl-label__optional">{optionalText}</span> : null}
    </Tag>
  );
}
