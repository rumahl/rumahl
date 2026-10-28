import type { ReactNode } from "react";
import "../routing/route-failure.css";

export function AppErrorCard({ title, message, actions, label }: {
  title: string;
  message: string;
  actions?: ReactNode;
  label?: string;
}) {
  return <section className="route-failure" role="status" aria-label={label ?? title}>
    <div className="route-failure__content">
      <div className="route-failure__icon" aria-hidden="true">
        <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="7" y="9" width="34" height="30" rx="7" />
          <path d="M7 18h34M13 13.5h.01M17 13.5h.01M24 24v6M24 34h.01" />
        </svg>
      </div>
      <div role="alert"><h2>{title}</h2><p>{message}</p></div>
      {actions ? <div className="route-failure__actions">{actions}</div> : null}
    </div>
  </section>;
}
