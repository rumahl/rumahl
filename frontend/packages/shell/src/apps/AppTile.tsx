import { GridIcon, SettingsIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { rememberRecent } from "./recents";
import type { ShellApp } from "./useShellApps";

export function AppIcon({ app }: { app: ShellApp }) {
  if (app.icon) return <img className="icon" src={app.icon} alt="" />;
  if (app.kind === "settings") return <SettingsIcon />;
  return <GridIcon />;
}

export function AppTile({ app, onNavigate }: { app: ShellApp; onNavigate?: ((path: string) => void) | undefined }) {
  const { t } = useI18n();
  return <ShellLink className="app-tile" data-app-id={app.id} to={app.path} onClick={() => { rememberRecent(app.path); onNavigate?.(app.path); }}>
    <span className={`app-tile__icon${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
    <span>{app.title}</span>{!app.launchable ? <small>{t("apps.notLaunchable")}</small> : null}
  </ShellLink>;
}
