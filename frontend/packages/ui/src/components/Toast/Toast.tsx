import type { ReactNode } from "react";

export type ToastTone = "info" | "success" | "warning" | "danger";

export interface ToastData {
  id: string;
  title?: ReactNode;
  message: ReactNode;
  tone?: ToastTone;
}

export interface ToastProps {
  title?: ReactNode;
  message: ReactNode;
  tone?: ToastTone | undefined;
  onClose?: () => void;
  closeLabel?: string;
  className?: string;
}

const SYMBOL: Record<ToastTone, string> = { info: "i", success: "✓", warning: "!", danger: "!" };

/** A single transient notification. Pair with {@link ToastStack}. */
export function Toast({ title, message, tone = "info", onClose, closeLabel = "Dismiss", className = "" }: ToastProps) {
  return (
    <div className={`rumahl-toast ${className}`} data-tone={tone} role="status">
      <span className="rumahl-toast__icon" aria-hidden="true">
        {SYMBOL[tone]}
      </span>
      <div className="rumahl-toast__body">
        {title ? <div className="rumahl-toast__title">{title}</div> : null}
        <div className="rumahl-toast__message">{message}</div>
      </div>
      {onClose ? (
        <button type="button" className="rumahl-toast__close" aria-label={closeLabel} onClick={onClose}>
          ×
        </button>
      ) : null}
    </div>
  );
}

export interface ToastStackProps {
  toasts: readonly ToastData[];
  onClose?: (id: string) => void;
  closeLabel?: string;
}

/** Fixed bottom-end stack of toasts (aria-live region). */
export function ToastStack({ toasts, onClose, closeLabel }: ToastStackProps) {
  return (
    <div className="rumahl-toast-stack" aria-live="polite" aria-relevant="additions">
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          title={toast.title}
          message={toast.message}
          tone={toast.tone}
          {...(onClose ? { onClose: () => onClose(toast.id) } : {})}
          {...(closeLabel ? { closeLabel } : {})}
        />
      ))}
    </div>
  );
}
