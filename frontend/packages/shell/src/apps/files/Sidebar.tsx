import { GridIcon, SettingsIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { DesktopArtwork, FolderIcon } from "./icons";
import { freshHost, freshStore, type HostArea, type Nav } from "./types";

export function Sidebar({ nav, advanced, onNavigate, onOpenDesktop, inert = false }: { nav: Nav; advanced: boolean; onNavigate: (next: Nav) => void; onOpenDesktop: () => void; inert?: boolean }) {
  const { t } = useI18n();
  const isStore = nav.place.kind === "store";
  const isDesktop = nav.place.kind === "desktop";
  const isHost = (area: HostArea) => nav.place.kind === "host" && nav.place.area === area;
  const item = (active: boolean) => `files-nav${active ? " is-active" : ""}`;
  return <aside className="files-app__sidebar" aria-label={t("files.locations")} inert={inert}>
    <h2 className="visually-hidden">{t("files.locations")}</h2>
    <nav aria-label={t("files.locations")}>
      <h3 className="files-app__heading">{t("files.personalGroup")}</h3>
      <button type="button" className={item(isStore)} aria-current={isStore ? "page" : undefined} onClick={() => onNavigate(freshStore())}>
        <FolderIcon className="files-nav__icon" /><span className="files-nav__label">{t("files.root")}</span>
      </button>
      <button type="button" className={item(isDesktop)} aria-current={isDesktop ? "page" : undefined} onClick={onOpenDesktop}>
        <DesktopArtwork className="files-nav__icon files-nav__icon--art" /><span className="files-nav__label">{t("files.desktop")}</span>
      </button>
      {advanced
        ? <>
            <div className="files-nav__section-heading"><h3 className="files-app__heading">{t("files.deviceGroup")}</h3></div>
            <button type="button" className={item(isHost("home"))} aria-current={isHost("home") ? "page" : undefined} onClick={() => onNavigate(freshHost("home"))}>
              <FolderIcon className="files-nav__icon" /><span className="files-nav__label">{t("files.home")}</span>
            </button>
            <div className="files-nav__section-heading"><span>{t("files.readOnly")}</span></div>
            <button type="button" className={item(isHost("apps"))} aria-current={isHost("apps") ? "page" : undefined} onClick={() => onNavigate(freshHost("apps"))}>
              <GridIcon className="files-nav__icon" /><span className="files-nav__label">{t("files.applications")}</span>
            </button>
            <button type="button" className={item(isHost("system"))} aria-current={isHost("system") ? "page" : undefined} onClick={() => onNavigate(freshHost("system"))}>
              <SettingsIcon className="files-nav__icon" /><span className="files-nav__label">{t("files.system")}</span>
            </button>
          </>
        : null}
    </nav>
  </aside>;
}
