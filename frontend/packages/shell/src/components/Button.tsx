import { RumahlTooltip } from "./RumahlTooltip";
import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

/** Shared class names so links and buttons look identical. */
export function buttonClass(variant: ButtonVariant = "secondary", size: ButtonSize = "md", extra?: string): string {
  return ["btn", `btn--${variant}`, `btn--${size}`, extra].filter(Boolean).join(" ");
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

export function Button({ variant = "secondary", size = "md", className, type, title, ...props }: ButtonProps) {
  const button = <button type={type ?? "button"} className={buttonClass(variant, size, className)} {...props} />;
  return title ? <RumahlTooltip content={title}>{button}</RumahlTooltip> : button;
}

export interface ButtonLinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  children: ReactNode;
}

export function ButtonLink({ variant = "secondary", size = "md", className, children, ...props }: ButtonLinkProps) {
  return <a className={buttonClass(variant, size, className)} {...props}>{children}</a>;
}
