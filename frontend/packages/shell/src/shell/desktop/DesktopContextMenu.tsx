import { useEffect } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../i18n";

export interface DesktopMenuState {
  x: number;
  y: number;
  appId?: string;
}

export interface DesktopContextMenuProps {
  menu: DesktopMenuState | null;
  appTitle?: string | undefined;
  hidden: readonly { id: string; title: string }[];
  widgets: boolean;
  onClose: () => void;
  onOpen: (appId: string) => void;
  onRemove: (appId: string) => void;
  onRestoreApp: (appId: string) => void;
  onSave: () => void;
  onRestoreLayout: () => void;
  onShowDesktop: () => void;
  onResetLayout: () => void;
  onToggleWidgets: () => void;
  onOpenSettings: () => void;
}

export function DesktopContextMenu(props: DesktopContextMenuProps) {
  const { t } = useI18n();
  const { menu, onClose } = props;
  useEffect(() => {
    if (!menu) return;
    const close = () => onClose();
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [menu, onClose]);
  if (!menu) return null;
  if (typeof document === "undefined") return null;
  const left = Math.min(menu.x, Math.max(8, window.innerWidth - 240));
  const top = Math.min(menu.y, Math.max(8, window.innerHeight - 300));
  return createPortal(
    <div className="desktop-menu" role="menu" aria-label={t("desktop.menu.label")}
      style={{ left, top }} onPointerDown={(event) => event.stopPropagation()}>
      {menu.appId ? <>
        <p className="desktop-menu__title">{props.appTitle}</p>
        <button role="menuitem" type="button" onClick={() => { props.onOpen(menu.appId!); onClose(); }}>{t("desktop.menu.open")}</button>
        <button role="menuitem" type="button" onClick={() => { props.onRemove(menu.appId!); onClose(); }}>{t("desktop.menu.remove")}</button>
      </> : <>
        <button role="menuitem" type="button" onClick={() => { props.onSave(); onClose(); }}>{t("desktop.menu.save")}</button>
        <button role="menuitem" type="button" onClick={() => { props.onRestoreLayout(); onClose(); }}>{t("desktop.menu.restore")}</button>
        <button role="menuitem" type="button" onClick={() => { props.onShowDesktop(); onClose(); }}>{t("desktop.menu.show")}</button>
        <button role="menuitemcheckbox" aria-checked={props.widgets} type="button" onClick={() => { props.onToggleWidgets(); onClose(); }}>{t("desktop.menu.widgets")}</button>
        <div className="desktop-menu__separator" role="separator" />
        <button role="menuitem" type="button" onClick={() => { props.onResetLayout(); onClose(); }}>{t("desktop.menu.reset")}</button>
      </>}
      {props.hidden.length > 0 ? <>
        <div className="desktop-menu__separator" role="separator" />
        <p className="desktop-menu__title">{t("desktop.menu.hidden")}</p>
        {props.hidden.map((app) => <button role="menuitem" type="button" key={app.id}
          onClick={() => { props.onRestoreApp(app.id); onClose(); }}>{t("desktop.menu.restoreApp", { title: app.title })}</button>)}
      </> : null}
      <div className="desktop-menu__separator" role="separator" />
      <button role="menuitem" type="button" onClick={() => { props.onOpenSettings(); onClose(); }}>{t("desktop.menu.settings")}</button>
    </div>,
    document.body
  );
}
