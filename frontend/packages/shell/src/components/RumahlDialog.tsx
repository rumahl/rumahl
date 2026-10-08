import { useEffect, useId, useRef, type ReactNode } from "react";
import { RumahlMark } from "./RumahlMark";
import { Button } from "./Button";

export interface RumahlModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  application?: string;
  closeLabel: string;
  children: ReactNode;
  actions?: ReactNode;
  description?: string;
  kind?: "window" | "alert";
}

/** Native top-layer window: background inertness, Escape and focus restoration. */
export function RumahlModal({
  open,
  onClose,
  title,
  application,
  closeLabel,
  children,
  actions,
  description,
  kind = "window",
}: RumahlModalProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog || !open) return;
    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    dialog.showModal();
    // Keep a system prompt associated with its owning window, clamped to the viewport.
    const owner = dialog.closest<HTMLElement>(".shell-window");
    const position = () => {
      if (!owner) return;
      const bounds = owner.getBoundingClientRect();
      const width = dialog.offsetWidth,
        height = dialog.offsetHeight;
      dialog.style.margin = "0";
      dialog.style.inset = "auto";
      dialog.style.left = `${Math.max(16, Math.min(bounds.left + (bounds.width - width) / 2, window.innerWidth - width - 16))}px`;
      dialog.style.top = `${Math.max(16, Math.min(bounds.top + (bounds.height - height) / 2, window.innerHeight - height - 16))}px`;
    };
    position();
    const observer =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(position)
        : null;
    if (owner) observer?.observe(owner);
    observer?.observe(dialog);
    window.addEventListener("resize", position);
    dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", position);
      dialog.close();
      if (previous?.isConnected) previous.focus();
    };
  }, [open]);
  if (!open) return null;
  return (
    <dialog
      ref={ref}
      className={`rumahl-modal rumahl-modal--${kind}`}
      role={kind === "alert" ? "alertdialog" : "dialog"}
      aria-modal="true"
      aria-labelledby={`${id}-title`}
      aria-describedby={description ? `${id}-description` : undefined}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      {kind === "window" ? (
        <header className="rumahl-modal__titlebar">
          <span className="rumahl-modal__identity">
            <RumahlMark />
            <span>{application ?? title}</span>
          </span>
          <button
            type="button"
            className="rumahl-modal__close"
            aria-label={closeLabel}
            onClick={onClose}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="m5 5 6 6m0-6-6 6" />
            </svg>
          </button>
        </header>
      ) : (
        <div className="rumahl-modal__identity rumahl-modal__identity--alert">
          <RumahlMark />
          <span>{application ?? title}</span>
        </div>
      )}
      <div className="rumahl-modal__body">
        <h2 id={`${id}-title`}>{title}</h2>
        {description ? <p id={`${id}-description`}>{description}</p> : null}
        {children}
      </div>
      {actions ? (
        <footer className="rumahl-modal__actions">{actions}</footer>
      ) : null}
    </dialog>
  );
}

/** A focused OS confirmation with a safe initial action and explicit outcome. */
export function RumahlDialog({
  open,
  onClose,
  onConfirm,
  title,
  description,
  confirmLabel,
  cancelLabel,
  application,
  destructive = false,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  application?: string;
  destructive?: boolean;
}) {
  return (
    <RumahlModal
      open={open}
      onClose={onClose}
      title={title}
      description={description}
      closeLabel={cancelLabel}
      {...(application ? { application } : {})}
      kind="alert"
      actions={
        <>
          <Button data-autofocus onClick={onClose}>
            {cancelLabel}
          </Button>
          <Button
            variant={destructive ? "danger" : "primary"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <span
        className={`rumahl-dialog__symbol${destructive ? " rumahl-dialog__symbol--danger" : ""}`}
        aria-hidden="true"
      >
        {destructive ? "!" : "?"}
      </span>
    </RumahlModal>
  );
}
