import { createContext, useContext, useEffect, useState, type PropsWithChildren, type CSSProperties, type PointerEvent as ReactPointerEvent, type PointerEventHandler, type KeyboardEventHandler, type ReactNode } from "react";
import type { WindowChromeVariant } from "@rumahl/contracts";
import { useTheme } from "@rumahl/ui";
import { renderSlot, type SlotBindings, type SlotComponentProps } from "@rumahl/ui/slots";
import { useI18n } from "../i18n";
import { WindowCloseIcon, WindowExitFullscreenIcon, WindowFullscreenIcon, WindowMaximizeIcon, WindowMinimizeIcon, WindowRestoreIcon } from "../icons";

const AppTopbarHostContext = createContext<HTMLElement | null>(null);

/** DOM host inside a window titlebar for a first-party app's custom topbar. */
export function useAppTopbarHost(): HTMLElement | null {
  return useContext(AppTopbarHostContext);
}

interface ProtectedWindowProps extends PropsWithChildren {
  focused: boolean;
  /** Let the routed first-party app render its own topbar into this window. */
  customTopbar?: boolean;
  frameless?: boolean;
  /** Hosted iframe app: the content fills the window with no padding. */
  flush?: boolean;
  style?: CSSProperties | undefined;
  maximized?: boolean;
  fullscreen?: boolean;
  onSnap?: ((side: "left" | "right") => void) | undefined;
  onMaximize?: (() => void) | undefined;
  onFullscreen?: (() => void) | undefined;
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
  children, customTopbar = false, style, frameless, flush = false, maximized, fullscreen, onMaximize, onFullscreen, onSnap, onTitlePointerDown, onTitleKeyDown, onResizePointerDown, onResizeKeyDown,
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
  const [topbarHost, setTopbarHost] = useState<HTMLElement | null>(null);
  const [topbarReady, setTopbarReady] = useState(false);
  useEffect(() => {
    if (!customTopbar || !topbarHost) {
      setTopbarReady(false);
      return;
    }
    const update = () => setTopbarReady(topbarHost.childElementCount > 0);
    update();
    const observer = new MutationObserver(update);
    observer.observe(topbarHost, { childList: true });
    return () => observer.disconnect();
  }, [customTopbar, topbarHost]);
  // Portal content sits in the app's React tree, so React's synthetic bubble
  // from the header never sees it. Bind window dragging directly to the host.
  useEffect(() => {
    if (!customTopbar || !topbarHost) return;
    const down = (event: globalThis.PointerEvent) => onTitlePointerDown?.(event as unknown as ReactPointerEvent<HTMLElement>);
    const double = (event: MouseEvent) => {
      if (!(event.target as HTMLElement).closest("button, input, select, textarea, a")) onMaximize?.();
    };
    topbarHost.addEventListener("pointerdown", down);
    topbarHost.addEventListener("dblclick", double);
    return () => {
      topbarHost.removeEventListener("pointerdown", down);
      topbarHost.removeEventListener("dblclick", double);
    };
  }, [customTopbar, topbarHost, onTitlePointerDown, onMaximize]);
  const controlButton = (variantName: string, label: string, action: () => void, icon: ReactNode, extra?: string) => (
    <button className={`window-control window-control--${variantName}${extra ? ` ${extra}` : ""}`} aria-label={label} onClick={action} type="button">{icon}</button>
  );
  const titleBlock = <div className="titlebar-titles">
    <h2 className="titlebar-title">{title}</h2>
    {subtitle ? <p className="titlebar-subtitle">{subtitle}</p> : null}
  </div>;
  const maximizeButton = onMaximize ? controlButton("maximize", t(maximized ? "window.restore" : "window.maximize", { title }), onMaximize, maximized ? <WindowRestoreIcon aria-hidden="true" /> : <WindowMaximizeIcon aria-hidden="true" />) : null;
  const fullscreenButton = onFullscreen ? controlButton("fullscreen", t(fullscreen ? "window.exitFullscreen" : "window.fullscreen", { title }), onFullscreen, fullscreen ? <WindowExitFullscreenIcon aria-hidden="true" /> : <WindowFullscreenIcon aria-hidden="true" />) : null;
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
    {fullscreenButton}
    {controlButton("close", t("window.close", { title }), onClose, <WindowCloseIcon aria-hidden="true" />)}
  </div>;
  const windowSlot = theme.slots?.window;
  const bindings: SlotBindings = {
    data: { title, subtitle, focused, maximized: maximized === true, frameless: frameless === true },
    actions: {
      close: onClose,
      minimize: onMinimize,
      maximize: () => onMaximize?.(),
      fullscreen: () => onFullscreen?.(),
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
    <AppTopbarHostContext value={topbarHost}>
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
      <header hidden={frameless} className="titlebar shell-window__titlebar" data-custom={customTopbar ? "" : undefined} tabIndex={onTitleKeyDown ? 0 : undefined}
        aria-label={onTitleKeyDown ? t("window.move", { title }) : undefined}
        onPointerDown={onTitlePointerDown} onKeyDown={onTitleKeyDown}
        onDoubleClick={(event) => { if (!(event.target as HTMLElement).closest("button, input, select, textarea, a")) onMaximize?.(); }}>
        {customTopbar ? <>
          {topbarReady ? null : <div className="shell-window__topbar-fallback">{titleBlock}</div>}
          <div className="shell-window__app-topbar" ref={setTopbarHost} />
          {controlsBlock}
        </> : windowSlot ? renderSlot(windowSlot, bindings) : <>{titleBlock}{controlsBlock}</>}
      </header>
      <div className="shell-window__content">{children}</div>
      {onResizePointerDown ? <button hidden={frameless || maximized} className="window-resize" aria-label={t("window.resize", { title })} onPointerDown={onResizePointerDown} onKeyDown={onResizeKeyDown} type="button" /> : null}
    </section>
    </AppTopbarHostContext>
  );
}
