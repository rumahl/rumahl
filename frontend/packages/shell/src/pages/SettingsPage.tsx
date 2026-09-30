import { themes } from "@rumahl/ui/themes";
import { useTheme } from "@rumahl/ui";
import type { ReactNode } from "react";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { WorkspaceSettings } from "../preferences/WorkspaceSettings";
import { useLocation, useRoutes } from "react-router";
import { DisplaySettings } from "./DisplaySettings";
import { SystemInfo } from "./SystemInfo";
import { RumahlMark } from "../components/RumahlMark";
import { SettingsHeading } from "../components/SettingsHeading";
import { ButtonLink } from "../components/Button";
import { SettingsIcon, DesktopIcon, GridIcon, PulseIcon, ShieldIcon, InfoIcon, DocumentIcon, ExternalLinkIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { UnavailablePage } from "./UnavailablePage";
import { useThemeTuning } from "../preferences/theme-tuning";
import { useShell } from "../shell/ShellContext";

const WEBSITE_URL = "https://rumahl.dev";
const SOURCE_URL = "https://github.com/rumahl/rumahl";
const APACHE_URL = "https://www.apache.org/licenses/LICENSE-2.0";
const MOTION_URL = "https://motion.dev";
const REACT_URL = "https://react.dev";
const ROUTER_URL = "https://reactrouter.com";

export function SettingsPage() {
  const { t } = useI18n();
  const { pathname } = useLocation();
  const preferences = useShellPreferences();
  const { theme } = useTheme();
  const content = useRoutes([
    { index: true, element: <SettingsOverview /> },
    { path: "display", Component: DisplaySettings },
    { path: "workspace", Component: WorkspaceSettings },
    { path: "accessibility", Component: AccessibilitySettings },
    { path: "system", Component: SystemSettings },
    { path: "system/about", Component: AboutSettings },
    { path: "system/licenses", Component: LicenseSettings },
    { path: "*", Component: UnavailablePage }
  ]);
  return <div className="window-body settings-layout"><aside className="sidebar settings-sidebar">
    <div className="side-profile settings-identity"><span className="brandmark" aria-hidden="true"><RumahlMark /></span><div><strong>rumahl OS</strong><div className="small">{t(`mode.${preferences.mode}`)} · {themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div></div>
    <div className="nav-heading">{t("preferences.user")}</div>
    <nav aria-label={t("nav.settings")}>
      <ShellLink className="nav-item" to="/settings/display" aria-current={pathname.startsWith("/settings/display") ? "page" : undefined}><DesktopIcon /><span>{t("settings.personalization")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/workspace" aria-current={pathname.startsWith("/settings/workspace") ? "page" : undefined}><GridIcon /><span>{t("workspace.title")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/accessibility" aria-current={pathname.startsWith("/settings/accessibility") ? "page" : undefined}><PulseIcon /><span>{t("settings.accessibility")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/system" aria-current={pathname.startsWith("/settings/system") ? "page" : undefined}><ShieldIcon /><span>{t("settings.system")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings" aria-current={pathname === "/settings" ? "page" : undefined}><SettingsIcon /><span>{t("nav.settings")}</span></ShellLink>
    </nav>
    <div className="side-bottom"><div className="build">rumahl OS <b>Preview</b><br />{themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div>
  </aside><div className="content settings-content" key={pathname}>{content}</div></div>;
}

function SettingsOverview() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { theme } = useTheme();
  return <section className="display-settings">
    <header className="settings-page-heading"><h1 className="pagetitle">{t("nav.settings")}</h1><p className="pagedesc">{t("settings.overviewHelp")}</p></header>
    <div className="settings-group settings-nav">
      <NavRow to="/settings/display" icon={<DesktopIcon />} title={t("settings.personalization")} subtitle={`${t(`mode.${settings.mode}`)} · ${themes.find(item => item.id === theme.id)?.name ?? theme.name}`} />
      <NavRow to="/settings/workspace" icon={<GridIcon />} title={t("workspace.title")} subtitle={t("settings.workspaceHelp")} />
      <NavRow to="/settings/accessibility" icon={<PulseIcon />} title={t("settings.accessibility")} subtitle={t("settings.accessibilityHelp")} />
      <NavRow to="/settings/system" icon={<ShieldIcon />} title={t("settings.system")} subtitle={t("settings.systemHelp")} />
    </div>
  </section>;
}

function AccessibilitySettings() {
  const { t } = useI18n();
  const { tuning, setAnimations, setWallpaperMotion } = useThemeTuning();
  const perf = tuning.performanceMode === true;
  return <section className="display-settings">
    <SettingsHeading title={t("settings.accessibility")} help={t("settings.accessibilityHelp")} back="/settings" />
    <h3 className="section-title">{t("theme.performance.title")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("theme.performance.animations")}</strong><p>{t("theme.performance.animationsHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.performance.animations")} disabled={perf} checked={tuning.animations !== false} onChange={event => setAnimations(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.wallpaperMotion")}</strong><p>{t("theme.wallpaperMotionHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.wallpaperMotion")} disabled={perf} checked={tuning.wallpaperMotion !== false} onChange={event => setWallpaperMotion(event.target.checked)} /></div>
      </div>
    </div>
  </section>;
}

function SystemSettings() {
  const { t } = useI18n();
  return <section className="display-settings">
    <SettingsHeading title={t("settings.system")} help={t("settings.systemHelp")} back="/settings" />
    <SystemInfo />
    <h3 className="section-title">{t("settings.system.sections")}</h3>
    <div className="settings-group settings-nav">
      <NavRow to="/settings/system/about" icon={<InfoIcon />} title={t("settings.system.about")} subtitle={t("settings.system.aboutHelp")} />
      <NavRow to="/settings/system/licenses" icon={<DocumentIcon />} title={t("settings.system.licenses")} subtitle={t("settings.system.licensesHelp")} />
    </div>
  </section>;
}

function AboutSettings() {
  const { t } = useI18n();
  const { snapshot, mode } = useShell();
  const { theme } = useTheme();
  return <section className="display-settings">
    <SettingsHeading title={t("settings.system.about")} help={t("settings.system.aboutHelp")} back="/settings/system" />

    <div className="settings-group">
      <div className="setting about-identity">
        <span className="about-identity__mark" aria-hidden="true"><RumahlMark /></span>
        <div className="copy"><strong>rumahl OS</strong><p>{t("settings.system.tagline")}</p></div>
      </div>
    </div>

    <h3 className="section-title">{t("settings.system.session")}</h3>
    <div className="settings-group">
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.user")}</strong></div><div className="visual"><span>{snapshot.user.displayName}</span></div></div>
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.version")}</strong></div><div className="visual"><code>{snapshot.shellBuildId.slice(0, 12)}</code></div></div>
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.mode")}</strong></div><div className="visual"><span>{t(`mode.${mode}`)}</span></div></div>
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.theme")}</strong></div><div className="visual"><span>{theme.name}</span></div></div>
    </div>

    <h3 className="section-title">{t("settings.system.legal")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("settings.system.license")}</strong><p>{t("settings.system.licenseNotice")}</p></div>
        <div className="visual"><ButtonLink size="sm" href={APACHE_URL} target="_blank" rel="noreferrer noopener">{t("settings.system.licenseName")}<ExternalLinkIcon /></ButtonLink></div>
      </div>
    </div>

    <h3 className="section-title">{t("settings.system.links")}</h3>
    <Links />
  </section>;
}

function LicenseSettings() {
  const { t } = useI18n();
  return <section className="display-settings">
    <SettingsHeading title={t("settings.system.licenses")} help={t("settings.system.licensesHelp")} back="/settings/system" />

    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("settings.system.licenseName")}</strong><p>{t("settings.system.licenseNotice")}</p></div>
        <div className="visual"><ButtonLink size="sm" href={APACHE_URL} target="_blank" rel="noreferrer noopener">{t("settings.system.open")}<ExternalLinkIcon /></ButtonLink></div>
      </div>
    </div>

    <h3 className="section-title">{t("settings.system.thirdParty")}</h3>
    <div className="settings-group settings-nav">
      <NavRow href={SOURCE_URL} title={t("settings.system.component.glass")} subtitle={t("settings.system.component.glassLicense")} />
      <NavRow href={MOTION_URL} title={t("settings.system.component.motion")} subtitle={t("settings.system.mitLicense")} />
      <NavRow href={REACT_URL} title={t("settings.system.component.react")} subtitle={t("settings.system.mitLicense")} />
      <NavRow href={ROUTER_URL} title={t("settings.system.component.router")} subtitle={t("settings.system.mitLicense")} />
    </div>
    <p className="settings-footnote">{t("settings.system.thirdPartyHelp")}</p>

    <h3 className="section-title">{t("settings.system.links")}</h3>
    <Links />
  </section>;
}

function Links() {
  const { t } = useI18n();
  return <div className="settings-group settings-nav">
    <NavRow href={WEBSITE_URL} icon={<InfoIcon />} title={t("settings.system.website")} subtitle="rumahl.dev" />
    <NavRow href={SOURCE_URL} icon={<DocumentIcon />} title={t("settings.system.source")} subtitle="github.com/rumahl/rumahl" />
    <NavRow href={APACHE_URL} icon={<ExternalLinkIcon />} title={t("settings.system.license")} subtitle={t("settings.system.licenseName")} />
  </div>;
}

function NavRow({ to, href, icon, title, subtitle }: { to?: string; href?: string; icon?: ReactNode; title: string; subtitle?: string }) {
  const external = href !== undefined;
  const body = <>
    {icon ? <span className="settings-nav__icon" aria-hidden="true">{icon}</span> : null}
    <span className="settings-nav__text"><strong>{title}</strong>{subtitle ? <small>{subtitle}</small> : null}</span>
    {external ? <ExternalLinkIcon className="settings-nav__ext" /> : <span className="settings-nav__chevron" aria-hidden="true">›</span>}
  </>;
  return external
    ? <a className="settings-nav__row" href={href} target="_blank" rel="noreferrer noopener">{body}</a>
    : <ShellLink className="settings-nav__row" to={to!}>{body}</ShellLink>;
}
