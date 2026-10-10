import type { PropsWithChildren, CSSProperties, PointerEventHandler, KeyboardEventHandler, ReactNode } from "react";
import type { WindowChromeVariant } from "@rumahl/contracts";
import { useTheme } from "@rumahl/ui";
import { renderSlot, type SlotBindings, type SlotComponentProps } from "@rumahl/ui/slots";
import { useI18n } from "../i18n";
import { WindowCloseIcon, WindowMaximizeIcon, WindowMinimizeIcon, WindowRestoreIcon } from "../icons";

interface ProtectedWindowProps extends PropsWithChildren {
  focused: boolean;
  frameless?: boolean;
  /** Hosted iframe app: the content fills the window with no padding. */
  flush?: boolean;
  style?: CSSProperties | undefined;
  maximized?: boolean;
  onSnap?: ((side: "left" | "right") => void) | undefined;
  onMaximize?: (() => void) | undefined;
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
  children, style, frameless, flush = false, maximized, onMaximize, onSnap, onTitlePointerDown, onTitleKeyDown, onResizePointerDown, onResizeKeyDown,
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
  const { theme } = useTheme();
  const controlButton = (variantName: string, label: string, action: () => void, icon: ReactNode, extra?: string) => (
    <button className={`window-control window-control--${variantName}${extra ? ` ${extra}` : ""}`} aria-label={label} onClick={action} type="button">{icon}</button>
  );
  const titleBlock = <div className="titlebar-titles">
    <h2 className="titlebar-title">{title}</h2>
    {subtitle ? <p className="titlebar-subtitle">{subtitle}</p> : null}
  </div>;
  const maximizeButton = onMaximize ? controlButton("maximize", t(maximized ? "window.restore" : "window.maximize", { title }), onMaximize, maximized ? <WindowRestoreIcon aria-hidden="true" /> : <WindowMaximizeIcon aria-hidden="true" />) : null;
  const controlsBlock = <div aria-label={t("window.controls")} className="winbuttons shell-window__controls">
    {controlButton("minimize", t("window.minimize", { title }), onMinimize, <WindowMinimizeIcon aria-hidden="true" />)}
    {onMaximize && onSnap
      ? <div className="window-control-group">
          {maximizeButton}
          <div className="window-snap-flyout" role="group" aria-label={t("window.controls")}>
            {controlButton("snap", t("window.snapLeft", { title }), () => onSnap("left"), "◧")}
            {controlButton("snap", t("window.snapRight", { title }), () => onSnap("right"), "◨")}
          </div>
        </div>
      : maximizeButton}
    {controlButton("close", t("window.close", { title }), onClose, <WindowCloseIcon aria-hidden="true" />)}
  </div>;
  const windowSlot = theme.slots?.window;
  const bindings: SlotBindings = {
    data: { title, subtitle, focused, maximized: maximized === true, frameless: frameless === true },
    actions: {
      close: onClose,
      minimize: onMinimize,
      maximize: () => onMaximize?.(),
      "snap-left": () => onSnap?.("left"),
      "snap-right": () => onSnap?.("right")
    },
    components: {
      title: (props: SlotComponentProps) => <h2 className={(props.className as string | undefined) ?? "titlebar-title"}>{title}</h2>,
      subtitle: (props: SlotComponentProps) => <p className={(props.className as string | undefined) ?? "titlebar-subtitle"}>{subtitle}</p>,
      controls: () => controlsBlock
    }
  };
  return (
    <section
      aria-label={title}
      className={`window shell-window shell-window--${variant}${stream ? " shell-window--stream" : ""}${flush ? " shell-window--flush" : ""}${focused ? " is-focused" : ""}`}
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
        {windowSlot ? renderSlot(windowSlot, bindings) : <>{titleBlock}{controlsBlock}</>}
      </header>
      <div className="shell-window__content">{children}</div>
      {onResizePointerDown ? <button hidden={frameless || maximized} className="window-resize" aria-label={t("window.resize", { title })} onPointerDown={onResizePointerDown} onKeyDown={onResizeKeyDown} type="button" /> : null}
    </section>
  );
}
