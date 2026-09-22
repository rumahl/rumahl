import type { PropsWithChildren } from "react";
import type { WindowChromeVariant } from "@rumahl/contracts";
import { useI18n } from "../i18n";

interface ProtectedWindowProps extends PropsWithChildren {
  focused: boolean;
  id: string;
  onClose: () => void;
  onFocus: () => void;
  onMinimize: () => void;
  subtitle: string;
  title: string;
  variant: WindowChromeVariant;
}

export function ProtectedWindow({
  children,
  focused,
  id,
  onClose,
  onFocus,
  onMinimize,
  subtitle,
  title,
  variant
}: ProtectedWindowProps) {
  const { t } = useI18n();
  return (
    <section
      aria-label={title}
      className={`shell-window shell-window--${variant}${focused ? " is-focused" : ""}`}
      data-window-id={id}
      onMouseDown={onFocus}
    >
      <header className="shell-window__titlebar">
        <div>
          <p>{subtitle}</p>
          <h2>{title}</h2>
        </div>
        <div aria-label={t("window.controls")} className="shell-window__controls">
          <button aria-label={t("window.minimize", { title })} onClick={onMinimize} type="button">
            <span aria-hidden="true" />
          </button>
          <button aria-label={t("window.close", { title })} onClick={onClose} type="button">
            <span aria-hidden="true" />
          </button>
        </div>
      </header>
      <div className="shell-window__content">{children}</div>
    </section>
  );
}
