import { RumahlSpinner } from "@rumahl/ui/components";

/**
 * Symbolic loading state.
 *
 * The shell says "working" with a spinner instead of a "Loading…" sentence; the
 * translated label stays available to assistive technology through the
 * spinner's status role, so nothing is lost for screen readers.
 */
export function LoadingStatus({ label, className }: { label: string; className?: string }) {
  return <span className={className ? `loading-status ${className}` : "loading-status"} data-loading="true">
    <RumahlSpinner size="sm" label={label} />
  </span>;
}
