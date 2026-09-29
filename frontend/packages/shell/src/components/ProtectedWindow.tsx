import type { PropsWithChildren, CSSProperties, PointerEventHandler, KeyboardEventHandler } from "react";
import type { WindowChromeVariant } from "@rumahl/contracts";
import { useI18n } from "../i18n";
import { WindowCloseIcon, WindowMaximizeIcon, WindowMinimizeIcon, WindowRestoreIcon } from "../icons";

interface ProtectedWindowProps extends PropsWithChildren {
  focused: boolean;
  frameless?: boolean;
  style?: CSSProperties | undefined;
  maximized?: boolean;
  onSnap?: (side: "left" | "right") => void;
  onMaximize?: () => void;
  onTitlePointerDown?: PointerEventHandler<HTMLElement>;
  onTitleKeyDown?: KeyboardEventHandler<HTMLElement>;
  onResizePointerDown?: PointerEventHandler<HTMLButtonElement>;
  onResizeKeyDown?: KeyboardEventHandler<HTMLButtonElement>;
  id: string;
  onClose: () => void;
  onFocus: () => void;
  onMinimize: () => void;
  subtitle: string;
  stream?: boolean;
  title: string;
  variant: WindowChromeVariant;
}

export function ProtectedWindow({
  children, style, frameless, maximized, onMaximize, onSnap, onTitlePointerDown, onTitleKeyDown, onResizePointerDown, onResizeKeyDown,
  focused,
  id,
  onClose,
  onFocus,
  onMinimize,
  subtitle,
  stream = false,
  title,
  variant
}: ProtectedWindowProps) {
  const { t } = useI18n();
  return (
    <section
      aria-label={title}
      className={`window shell-window shell-window--${variant}${stream ? " shell-window--stream" : ""}${focused ? " is-focused" : ""}`}
      data-window-id={id}
      ref={(element) => {
        // CSSOM property assignment works with the shell's strict style-src CSP;
        // an SSR style attribute would be blocked before hydration.
        if (element && style) for (const property of ["left", "top", "width", "height"] as const) {
          const value = style[property];
          element.style[property] = typeof value === "number" ? `${value}px` : value ?? "";
        }
      }}
      onPointerDown={onFocus}
      onFocusCapture={onFocus}
    >
      <header hidden={frameless} className="titlebar shell-window__titlebar" tabIndex={onTitleKeyDown ? 0 : undefined}
        aria-label={onTitleKeyDown ? t("window.move", { title }) : undefined}
        onPointerDown={onTitlePointerDown} onKeyDown={onTitleKeyDown}
        onDoubleClick={(event) => { if (!(event.target as HTMLElement).closest("button")) onMaximize?.(); }}>
        <div className="titlebar-titles">
          <h2 className="titlebar-title">{title}</h2>
          <p className="titlebar-subtitle">{subtitle}</p>
        </div>
        <div aria-label={t("window.controls")} className="winbuttons shell-window__controls">
          <button className="window-control window-control--minimize" aria-label={t("window.minimize", { title })} onClick={onMinimize} type="button">
            <WindowMinimizeIcon aria-hidden="true" />
          </button>
          {onSnap ? <><button className="window-control window-control--snap" type="button" aria-label={t("window.snapLeft", { title })} onClick={() => onSnap("left")}>◧</button><button className="window-control window-control--snap" type="button" aria-label={t("window.snapRight", { title })} onClick={() => onSnap("right")}>◨</button></> : null}
          {onMaximize ? <button className="window-control window-control--maximize" aria-label={t(maximized ? "window.restore" : "window.maximize", { title })} onClick={onMaximize} type="button">{maximized ? <WindowRestoreIcon aria-hidden="true" /> : <WindowMaximizeIcon aria-hidden="true" />}</button> : null}
          <button className="window-control window-control--close" aria-label={t("window.close", { title })} onClick={onClose} type="button">
            <WindowCloseIcon aria-hidden="true" />
          </button>
        </div>
      </header>
      <div className="shell-window__content">{children}</div>
      {onResizePointerDown ? <button hidden={frameless || maximized} className="window-resize" aria-label={t("window.resize", { title })} onPointerDown={onResizePointerDown} onKeyDown={onResizeKeyDown} type="button" /> : null}
    </section>
  );
}
