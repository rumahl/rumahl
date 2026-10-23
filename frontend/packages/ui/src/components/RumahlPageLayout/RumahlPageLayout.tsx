import type { ReactNode } from "react";

/** Shared presentation of the OS settings heading; routing stays with the host. */
/** Page heading with an optional back link and help text. */
export function RumahlPageHeading({ title, help, back, className = "rumahl-page-heading" }: {
  title: string;
  help?: string | undefined;
  back?: ReactNode;
  className?: string;
}) {
  return <header className={className}>
    <div className="settings-heading">
      {back}
      <h1 className="pagetitle">{title}</h1>
    </div>
    {help ? <p className="pagedesc">{help}</p> : null}
  </header>;
}

/** OS section placeholder, with optional actions for empty or initial states. */
/** Centred empty state with an eyebrow, title, body and actions. */
export function RumahlEmptyState({ eyebrow, title, children, action, className = "rumahl-empty-state" }: {
  eyebrow?: string;
  title: string;
  children: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return <main className={className}>
    {eyebrow ? <p className="eyebrow">{eyebrow}</p> : null}
    <h1>{title}</h1>
    <p>{children}</p>
    {action ? <div className="rumahl-empty-state__actions">{action}</div> : null}
  </main>;
}
