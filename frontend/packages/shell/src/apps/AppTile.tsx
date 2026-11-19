import { useEffect, useState } from "react";
import { SettingsIcon } from "../icons";
import filesAppIcon from "./files/folder-icons/folder-app.svg";
import petsAppIcon from "./pets/pet-icon.svg";
import defaultAppIcon from "./default-icons/default-app.svg";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { rememberRecent } from "./recents";
import type { ShellApp } from "./useShellApps";

export function AppIcon({ app }: { app: ShellApp }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => { setFailed(false); }, [app.icon]);
  if (app.icon && !failed) return <img className="icon" src={app.icon} alt="" onError={() => setFailed(true)} />;
  if (app.id === "files") return <span className="app-icon-folder" aria-hidden="true"><img className="app-icon-folder__art" src={filesAppIcon} alt="" draggable={false} /></span>;
  if (app.id === "pets") return <img className="app-icon-pets" src={petsAppIcon} alt="" draggable={false} />;
  return <img className="app-icon-default" src={defaultAppIcon} alt="" draggable={false} />;
}

export function AppTile({ app, onNavigate, minimal = false, status }: {
  app: ShellApp;
  onNavigate?: ((path: string) => void) | undefined;
  /** Launcher mode: no settings gear and no launchability caption. */
  minimal?: boolean;
  /** Optional live status dot shown in the icon's top-right corner. */
  status?: "running" | "stopped" | "unknown";
}) {
  const { t } = useI18n();
  const settingsTarget = app.managePath ?? app.settingsPath;
  return <div className="app-tile-item">
    <ShellLink className="app-tile" data-app-id={app.id} to={app.path} onClick={() => { rememberRecent(app.path); onNavigate?.(app.path); }}>
      <span className={`app-tile__icon${app.system ? " is-system" : ""}`} aria-hidden="true">
        <AppIcon app={app} />
        {status ? <i className={`app-tile__status app-tile__status--${status}`} /> : null}
      </span>
      <span>{app.title}</span>{!minimal && !app.launchable ? <small>{t("apps.notLaunchable")}</small> : null}
    </ShellLink>
    {!minimal && settingsTarget ? <ShellLink className="app-tile__settings" to={settingsTarget} aria-label={t("appSettings.open")} onClick={() => onNavigate?.(settingsTarget)}><SettingsIcon /></ShellLink> : null}
  </div>;
}
