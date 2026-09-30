import { useEffect, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";

export type ToastVariant = "success" | "warning" | "info" | "normal" | "error";
export interface ToastButton {
  label: string;
  onClick?: () => void;
  variant?: "primary" | "default";
}
export interface ToastOptions {
  title?: string;
  message: string;
  variant?: ToastVariant;
  /** Milliseconds before auto-dismiss; `null`/omitted keeps it persistent. */
  duration?: number | null;
  /** Up to two action buttons. */
  buttons?: ToastButton[];
  /** Provide an id to update/recall the same toast later. */
  id?: string;
}
export interface Toast extends ToastOptions {
  id: string;
  variant: ToastVariant;
}

let toasts: readonly Toast[] = [];
let animationsOn = true;
/** Enables/disables toast motion (performance mode / reduce animations). */
export function setToastAnimations(value: boolean): void { animationsOn = value; }
const listeners = new Set<() => void>();
let counter = 0;
const emit = () => { for (const listener of listeners) listener(); };

/** Shows a toast and returns its id (pass it back to `dismissToast`/`updateToast`). */
export function showToast(options: ToastOptions): string {
  const id = options.id ?? `toast-${++counter}`;
  const toast: Toast = { ...options, id, variant: options.variant ?? "normal", ...(options.buttons ? { buttons: options.buttons.slice(0, 2) } : {}) };
  toasts = [...toasts.filter((entry) => entry.id !== id), toast];
  emit();
  return id;
}
export function updateToast(id: string, patch: Partial<ToastOptions>): void {
  toasts = toasts.map((entry) => (entry.id === id ? { ...entry, ...patch, id } : entry));
  emit();
}
/** Dismisses a toast (by id) or all toasts when called without an id. */
export function dismissToast(id?: string): void {
  toasts = id === undefined ? [] : toasts.filter((entry) => entry.id !== id);
  emit();
}

function subscribe(listener: () => void): () => void { listeners.add(listener); return () => { listeners.delete(listener); }; }
function getSnapshot(): readonly Toast[] { return toasts; }
/** Current toasts (diagnostics). */
export function getToasts(): readonly Toast[] { return toasts; }
export function useToasts(): readonly Toast[] { return useSyncExternalStore(subscribe, getSnapshot, getSnapshot); }

/** Installs the global API so apps and the system can raise toasts. */
export function installToastApi(): void {
  if (typeof window === "undefined") return;
  (window as unknown as { rumahlToast?: unknown }).rumahlToast = {
    show: showToast,
    update: updateToast,
    dismiss: dismissToast
  };
}

function ToastItem({ toast }: { toast: Toast }) {
  useEffect(() => {
    if (toast.duration === null || toast.duration === undefined) return;
    const timer = setTimeout(() => dismissToast(toast.id), toast.duration);
    return () => clearTimeout(timer);
  }, [toast.id, toast.duration]);
  return <div className={`toast toast--${toast.variant}`} role="status">
    <div className="toast__head">
      <span className={`toast__dot toast__dot--${toast.variant}`} aria-hidden="true" />
      {toast.title ? <strong className="toast__title">{toast.title}</strong> : null}
      <button type="button" className="toast__close" aria-label="Dismiss" onClick={() => dismissToast(toast.id)}>✕</button>
    </div>
    <span className="toast__message">{toast.message}</span>
    {toast.buttons?.length ? <div className="toast__actions">
      {toast.buttons.map((button, index) => (
        <button key={index} type="button" className={button.variant === "primary" ? "toast__button toast__button--primary" : "toast__button"}
          onClick={() => { button.onClick?.(); }}>{button.label}</button>
      ))}
    </div> : null}
  </div>;
}

/** Stackable toast surface, top right. */
export function ToastStack() {
  const list = useToasts();
  const [host, setHost] = useState<HTMLElement | null>(null);
  useEffect(() => { setHost(document.body); }, []);
  if (!host || !list.length) return null;
  // At most two are expanded; the rest stack behind the second (with a count).
  const visible = list.slice(0, 2);
  const extra = list.length - visible.length;
  return createPortal(<div className="toast-stack">
    <AnimatePresence initial={false}>
      {visible.map((toast, index) => (
        <motion.div key={toast.id} className="toast-entry" layout={animationsOn}
          {...(animationsOn ? { initial: { opacity: 0, x: 24, scale: 0.97 }, exit: { opacity: 0, x: 24, scale: 0.97 } } : { initial: false })}
          animate={{ opacity: 1, x: 0, scale: 1 }}
          transition={{ duration: 0.22, ease: [0.2, 0.7, 0.2, 1] }}>
          {extra > 0 && index === visible.length - 1 ? <div className="toast-stack__ghosts" aria-hidden="true">
            <span className="toast-stack__ghost" />
            <span className="toast-stack__ghost" />
            <span className="toast-stack__count">+{extra}</span>
          </div> : null}
          <ToastItem toast={toast} />
        </motion.div>
      ))}
    </AnimatePresence>
  </div>, host);
}
