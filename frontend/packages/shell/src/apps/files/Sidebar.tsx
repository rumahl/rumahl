import { DesktopIcon, GridIcon, SettingsIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { FolderIcon } from "./icons";
import { freshHost, freshStore, type HostArea, type Nav } from "./types";

export function Sidebar({ nav, advanced, onNavigate, onOpenDesktop }: { nav: Nav; advanced: boolean; onNavigate: (next: Nav) => void; onOpenDesktop: () => void }) {
  const { t } = useI18n();
  const isStore = nav.place.kind === "store";
  const isHost = (area: HostArea) => nav.place.kind === "host" && nav.place.area === area;
  const isDesktop = isStore && nav.trail.at(-1)?.name === t("files.desktop");
  const item = (active: boolean) => `files-nav${active ? " is-active" : ""}`;
  return <aside className="files-app__sidebar" aria-label={t("files.locations")}>
    <p className="files-app__heading">{t("files.locations")}</p>
    <nav>
      <button type="button" className={item(isStore && !isDesktop)} aria-current={isStore && !isDesktop ? "page" : undefined} onClick={() => onNavigate(freshStore())}>
        <FolderIcon className="files-nav__icon" />{t("files.root")}
      </button>
      <button type="button" className={item(isDesktop)} aria-current={isDesktop ? "page" : undefined} onClick={onOpenDesktop}>
        <DesktopIcon className="files-nav__icon" />{t("files.desktop")}
      </button>
      {advanced
        ? <>
            <button type="button" className={item(isHost("apps"))} aria-current={isHost("apps") ? "page" : undefined} onClick={() => onNavigate(freshHost("apps"))}>
              <GridIcon className="files-nav__icon" />{t("files.applications")}
            </button>
            <button type="button" className={item(isHost("system"))} aria-current={isHost("system") ? "page" : undefined} onClick={() => onNavigate(freshHost("system"))}>
              <SettingsIcon className="files-nav__icon" />{t("files.system")}
            </button>
          </>
        : null}
    </nav>
  </aside>;
}
