import type { AnchorHTMLAttributes, ReactNode } from "react";
import { ExternalLinkIcon } from "../../icons";

export interface LinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  /** Adds `target="_blank"`, a safe `rel` and an external-link glyph. */
  external?: boolean;
  children?: ReactNode;
}

/**
 * Text link styled with the accent colour. Real `<a>`; keyboard and screen
 * reader friendly. `external` opens a new tab and appends an external glyph.
 */
export function Link({ external, className = "", rel, target, children, ...props }: LinkProps) {
  return (
    <a
      className={`rumahl-link ${className}`}
      {...(external ? { target: target ?? "_blank", rel: rel ?? "noreferrer noopener" } : {})}
      {...props}
    >
      {children}
      {external ? <ExternalLinkIcon className="rumahl-link__external" aria-hidden="true" /> : null}
    </a>
  );
}
