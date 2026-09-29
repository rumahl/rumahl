import { themes } from "@rumahl/ui/themes";
import { useTheme } from "@rumahl/ui";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { WorkspaceSettings } from "../preferences/WorkspaceSettings";
import { useLocation, useRoutes } from "react-router";
import { DisplaySettings } from "./DisplaySettings";
import { RumahlMark } from "../components/RumahlMark";
import { SettingsIcon, DesktopIcon, GridIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { UnavailablePage } from "./UnavailablePage";

export function SettingsPage() {
  const { t } = useI18n();
  const { pathname } = useLocation();
  const preferences = useShellPreferences();
  const { theme } = useTheme();
  const content = useRoutes([
    { index: true, element: <SettingsOverview /> },
    { path: "display", Component: DisplaySettings },
    { path: "workspace", Component: WorkspaceSettings },
    { path: "*", Component: UnavailablePage }
  ]);
  return <div className="window-body settings-layout"><aside className="sidebar settings-sidebar">
    <div className="side-profile settings-identity"><span className="brandmark" aria-hidden="true"><RumahlMark /></span><div><strong>rumahl OS</strong><div className="small">{t(`mode.${preferences.mode}`)} · {themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div></div>
    <div className="nav-heading">{t("preferences.user")}</div>
    <nav aria-label={t("nav.settings")}>
      <ShellLink className="nav-item" to="/settings" aria-current={pathname === "/settings" ? "page" : undefined}><SettingsIcon /><span>{t("nav.settings")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/display" aria-current={pathname.startsWith("/settings/display") ? "page" : undefined}><DesktopIcon /><span>{t("mode.label")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/workspace" aria-current={pathname.startsWith("/settings/workspace") ? "page" : undefined}><GridIcon /><span>{t("workspace.title")}</span></ShellLink>
    </nav>
    <div className="side-bottom"><div className="build">rumahl OS <b>Preview</b><br />{themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div>
  </aside><div className="content settings-content" key={pathname}>{content}</div></div>;
}

function SettingsOverview() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { theme } = useTheme();
  return <section className="settings-overview">
    <header className="settings-page-heading"><div className="heading-flex"><h1 className="pagetitle">{t("nav.settings")}</h1><span className="badge">rumahl OS</span></div><p className="pagedesc">{t("settings.overviewHelp")}</p></header>
    <div className="settings-overview-links">
      <ShellLink to="/settings/display"><DesktopIcon /><span><strong>{t("mode.label")}</strong><small>{t(`mode.${settings.mode}`)} · {themes.find(item => item.id === theme.id)?.name ?? theme.name}</small></span><span aria-hidden="true">›</span></ShellLink>
      <ShellLink to="/settings/workspace"><GridIcon /><span><strong>{t("workspace.title")}</strong><small>{t("settings.workspaceHelp")}</small></span><span aria-hidden="true">›</span></ShellLink>
    </div>
  </section>;
}
