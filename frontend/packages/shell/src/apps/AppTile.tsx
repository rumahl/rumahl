import { SettingsIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { rememberRecent } from "./recents";
import type { ShellApp } from "./useShellApps";

export function AppIcon({ app }: { app: ShellApp }) {
  if (app.icon) return <img className="icon" src={app.icon} alt="" />;
  if (app.kind === "settings") return <SettingsIcon />;
  const label = app.title.trim().slice(0, 1).toLocaleUpperCase() || "?";
  let hue = 0;
  for (let index = 0; index < app.id.length; index += 1) hue = (hue * 31 + app.id.charCodeAt(index)) % 360;
  return <span className="app-icon-glyph" aria-hidden="true"
    style={{ backgroundImage: `linear-gradient(150deg, hsl(${hue} 58% 54%), hsl(${(hue + 34) % 360} 52% 36%))` }}>{label}</span>;
}

export function AppTile({ app, onNavigate }: { app: ShellApp; onNavigate?: ((path: string) => void) | undefined }) {
  const { t } = useI18n();
  const settingsPath = `${app.path}/settings`;
  return <div className="app-tile-item">
    <ShellLink className="app-tile" data-app-id={app.id} to={app.path} onClick={() => { rememberRecent(app.path); onNavigate?.(app.path); }}>
      <span className={`app-tile__icon${app.system ? " is-system" : ""}`} aria-hidden="true"><AppIcon app={app} /></span>
      <span>{app.title}</span>{!app.launchable ? <small>{t("apps.notLaunchable")}</small> : null}
    </ShellLink>
    {!app.system ? <ShellLink className="app-tile__settings" to={settingsPath} aria-label={t("appSettings.open")} onClick={() => onNavigate?.(settingsPath)}><SettingsIcon /></ShellLink> : null}
  </div>;
}
