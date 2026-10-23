import type { ReactNode } from "react";
import { WindowCloseIcon, WindowMaximizeIcon, WindowMinimizeIcon, WindowRestoreIcon } from "../../icons";

export interface WindowControlLabels {
  controls?: string;
  minimize?: string;
  maximize?: string;
  restore?: string;
  close?: string;
}

export interface WindowControlsProps {
  onClose?: (() => void) | undefined;
  onMinimize?: (() => void) | undefined;
  onMaximize?: (() => void) | undefined;
  maximized?: boolean | undefined;
  labels?: WindowControlLabels | undefined;
}

function ControlButton({
  variant,
  label,
  onClick,
  children
}: {
  variant: string;
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className="rumahl-window__control" data-variant={variant} aria-label={label} onClick={onClick}>
      {children}
    </button>
  );
}

/** Minimize / maximize / close controls for a {@link Window}. */
export function WindowControls({ onClose, onMinimize, onMaximize, maximized, labels }: WindowControlsProps) {
  return (
    <div className="rumahl-window__controls" role="group" aria-label={labels?.controls ?? "Window controls"}>
      {onMinimize ? (
        <ControlButton variant="minimize" label={labels?.minimize ?? "Minimize"} onClick={onMinimize}>
          <WindowMinimizeIcon aria-hidden="true" />
        </ControlButton>
      ) : null}
      {onMaximize ? (
        <ControlButton
          variant="maximize"
          label={maximized ? (labels?.restore ?? "Restore") : (labels?.maximize ?? "Maximize")}
          onClick={onMaximize}
        >
          {maximized ? <WindowRestoreIcon aria-hidden="true" /> : <WindowMaximizeIcon aria-hidden="true" />}
        </ControlButton>
      ) : null}
      {onClose ? (
        <ControlButton variant="close" label={labels?.close ?? "Close"} onClick={onClose}>
          <WindowCloseIcon aria-hidden="true" />
        </ControlButton>
      ) : null}
    </div>
  );
}

/** Parts handed to a custom {@link WindowProps.titlebar} renderer. */
export interface WindowTitlebarParts {
  /** The default title + subtitle block (drop it if the app shows its own). */
  title: ReactNode;
  /** The wired window controls, to place anywhere in the custom topbar. */
  controls: ReactNode;
}

export interface WindowProps {
  title: ReactNode;
  subtitle?: ReactNode;
  focused?: boolean;
  maximized?: boolean;
  frameless?: boolean;
  /** Flush content: no content padding (hosted apps). */
  flush?: boolean;
  variant?: string;
  /** Extra titlebar content shown before the controls (default titlebar). */
  actions?: ReactNode;
  onClose?: () => void;
  onMinimize?: () => void;
  onMaximize?: () => void;
  labels?: WindowControlLabels;
  /**
   * Custom titlebar / topbar. The app rebuilds the topbar natively — a toolbar,
   * menus, a search field, tabs … — and places the wired `controls` wherever it
   * wants, like a custom title bar on Windows or macOS. The header keeps the
   * native height, padding and divider; `data-custom` is set for finer styling.
   * Combine with `frameless` for a fully bespoke shell.
   */
  titlebar?: (parts: WindowTitlebarParts) => ReactNode;
  children: ReactNode;
  className?: string;
}

/**
 * OS window frame: a titlebar with window controls around a content area.
 * Presentational — the caller owns dragging, resizing and stacking.
 */
export function Window({
  title,
  subtitle,
  focused,
  maximized,
  frameless,
  flush,
  variant = "standard",
  actions,
  onClose,
  onMinimize,
  onMaximize,
  labels,
  titlebar,
  children,
  className = ""
}: WindowProps) {
  const titleBlock = (
    <div className="rumahl-window__titles">
      <h2 className="rumahl-window__title">{title}</h2>
      {subtitle ? <p className="rumahl-window__subtitle">{subtitle}</p> : null}
    </div>
  );
  const controls = (
    <WindowControls
      onClose={onClose}
      onMinimize={onMinimize}
      onMaximize={onMaximize}
      maximized={maximized}
      labels={labels}
    />
  );
  return (
    <section
      aria-label={typeof title === "string" ? title : undefined}
      className={`rumahl-window rumahl-window-surface rumahl-window--${variant}${frameless ? " rumahl-window--frameless" : ""}${flush ? " rumahl-window--flush" : ""}${focused ? " is-focused" : ""} ${className}`}
    >
      {!frameless ? (
        <header className="rumahl-window__titlebar" data-custom={titlebar ? "" : undefined}>
          {titlebar ? (
            titlebar({ title: titleBlock, controls })
          ) : (
            <>
              {titleBlock}
              {actions}
              {controls}
            </>
          )}
        </header>
      ) : null}
      <div className="rumahl-window__content">{children}</div>
    </section>
  );
}
