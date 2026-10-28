import { AppIcon } from "../apps/AppTile";
import { CATALOG_REFRESH_EVENT, useAppCatalog } from "../apps/AppCatalog";
import { controlAppRuntime, fetchAppRuntime, fetchAppSettings, uninstallApp, type AppRuntime, type InstalledApp } from "../apps/client";
import { fetchAudit, fetchSensors, restartComponent, type AuditEvent, type Sensors } from "../apps/recovery-client";
import { useShellApps, type ShellApp } from "../apps/useShellApps";
import { appPath } from "../routing/paths";
import { useOsModePolicy } from "../preferences/OsMode";
import { setAppStatusIndicators, useAppStatusIndicators } from "../preferences/app-status-indicators";
import { setAppEnabled, useDisabledApps } from "../preferences/disabled-apps";
import { DeviceSettings } from "./DeviceSettings";
import { MobileSettings } from "./MobileSettings";
import { StoreSettings } from "./StoreSettings";
import { themes } from "@rumahl/ui/themes";
import { useTheme } from "@rumahl/ui";
import { useEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { WorkspaceSettings } from "../preferences/WorkspaceSettings";
import { useLocation, useParams, useRoutes } from "react-router";
import { DisplaySettings } from "./DisplaySettings";
import { SystemInfo } from "./SystemInfo";
import { OsModeSettings } from "./OsModeSettings";
import { RumahlMark } from "../components/RumahlMark";
import { SettingsHeading } from "../components/SettingsHeading";
import { Button, ButtonLink } from "../components/Button";
import { RumahlButtonGroup } from "../components/RumahlButtonGroup";
import { SettingsIcon, DesktopIcon, GridIcon, PulseIcon, ShieldIcon, InfoIcon, DocumentIcon, ExternalLinkIcon, PhoneIcon, StoreIcon, PlayIcon, StopIcon, RefreshIcon } from "../icons";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { UnavailablePage } from "./UnavailablePage";
import { useThemeTuning } from "../preferences/theme-tuning";
import { useShell } from "../shell/ShellContext";
import { osName, versionLabel } from "../system-labels";

const WEBSITE_URL = "https://rumahl.dev";
const SOURCE_URL = "https://github.com/rumahl/rumahl";
const APACHE_URL = "https://www.apache.org/licenses/LICENSE-2.0";
const MOTION_URL = "https://motion.dev";
const REACT_URL = "https://react.dev";
const ROUTER_URL = "https://reactrouter.com";

function clampNavWidth(value: number): number {
  return Math.min(420, Math.max(150, Math.round(value)));
}

export function SettingsPage() {
  const { t } = useI18n();
  const { pathname } = useLocation();
  const preferences = useShellPreferences();
  const { theme } = useTheme();
  const { snapshot, live, mode } = useShell();
  const [navWidth, setNavWidth] = useState<number | null>(null);
  const [resizing, setResizing] = useState(false);
  const stopResize = useRef<(() => void) | null>(null);
  useEffect(() => {
    try {
      const stored = Number(localStorage.getItem("rumahl.settings.sidenav-width"));
      if (Number.isFinite(stored) && stored > 0) setNavWidth(clampNavWidth(stored));
    } catch { /* storage may be unavailable */ }
    return () => stopResize.current?.();
  }, []);
  const persistNavWidth = (value: number) => {
    try { localStorage.setItem("rumahl.settings.sidenav-width", String(value)); } catch { /* ignore */ }
  };
  const applyNavWidth = (value: number) => {
    const next = clampNavWidth(value);
    setNavWidth(next);
    return next;
  };
  const beginResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = navWidth ?? event.currentTarget.parentElement?.querySelector<HTMLElement>(".settings-sidebar")?.getBoundingClientRect().width ?? 184;
    let next = startWidth;
    setResizing(true);
    const previousCursor = document.body.style.cursor;
    const previousSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    const move = (moveEvent: PointerEvent) => { next = applyNavWidth(startWidth + moveEvent.clientX - startX); };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousSelect;
      setResizing(false);
      persistNavWidth(next);
      stopResize.current = null;
    };
    stopResize.current = stop;
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  };
  const resizeWithKeyboard = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const current = navWidth ?? event.currentTarget.parentElement?.querySelector<HTMLElement>(".settings-sidebar")?.getBoundingClientRect().width ?? 184;
    const next = applyNavWidth(event.key === "Home" ? 150 : event.key === "End" ? 420 : current + (event.key === "ArrowRight" ? 12 : -12));
    persistNavWidth(next);
  };
  const content = useRoutes([
    { index: true, element: <SettingsOverview /> },
    { path: "device", Component: DeviceSettings },
    { path: "display", Component: DisplaySettings },
    { path: "workspace", Component: WorkspaceSettings },
    { path: "store", Component: StoreSettings },
    { path: "apps", Component: AppsSettings },
    { path: "apps/:appId", Component: AppManagement },
    { path: "accessibility", Component: AccessibilitySettings },
    { path: "mobile", Component: MobileSettings },
    { path: "os", Component: OsModeSettings },
    { path: "system", Component: SystemSettings },
    { path: "system/about", Component: AboutSettings },
    { path: "system/licenses", Component: LicenseSettings },
    { path: "*", Component: UnavailablePage }
  ]);
  const layoutStyle = navWidth === null ? undefined : ({ "--settings-nav-width": `${navWidth}px` } as CSSProperties);
  return <div className="window-body settings-layout" data-resizing={resizing ? "true" : undefined} style={layoutStyle}><aside className="sidebar settings-sidebar">
    <div className="side-profile settings-identity">
      <span className="settings-avatar" aria-hidden="true">{snapshot.user.displayName.slice(0, 1).toUpperCase()}</span>
      <div><strong>{snapshot.user.displayName}</strong><div className="small">{t(`mode.${preferences.mode}`)} · {themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div>
    </div>
    <div className="nav-heading">{t("preferences.user")}</div>
    <nav aria-label={t("nav.settings")}>
      <ShellLink className="nav-item" to="/settings/display" aria-current={pathname.startsWith("/settings/display") ? "page" : undefined}><DesktopIcon /><span>{t("settings.personalization")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/workspace" aria-current={pathname.startsWith("/settings/workspace") ? "page" : undefined}><GridIcon /><span>{t("workspace.title")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/store" aria-current={pathname.startsWith("/settings/store") ? "page" : undefined}><StoreIcon /><span>{t("storeSources.title")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/apps" aria-current={pathname.startsWith("/settings/apps") ? "page" : undefined}><GridIcon /><span>{t("settings.apps")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/accessibility" aria-current={pathname.startsWith("/settings/accessibility") ? "page" : undefined}><PulseIcon /><span>{t("settings.accessibility")}</span></ShellLink>
      {mode === "mobile" ? <ShellLink className="nav-item" to="/settings/mobile" aria-current={pathname.startsWith("/settings/mobile") ? "page" : undefined}><PhoneIcon /><span>{t("settings.mobile")}</span></ShellLink> : null}
      <ShellLink className="nav-item" to="/settings/os" aria-current={pathname.startsWith("/settings/os") ? "page" : undefined}><ShieldIcon /><span>{t("settings.osMode")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/device" aria-current={pathname.startsWith("/settings/device") ? "page" : undefined}><DesktopIcon /><span>{t("device.title")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings/system" aria-current={pathname.startsWith("/settings/system") ? "page" : undefined}><ShieldIcon /><span>{t("settings.system")}</span></ShellLink>
      <ShellLink className="nav-item" to="/settings" aria-current={pathname === "/settings" ? "page" : undefined}><SettingsIcon /><span>{t("nav.settings")}</span></ShellLink>
    </nav>
    <div className="side-bottom"><div className="build">{live?.demo ? osName(true) : <>rumahl OS <b>Preview</b></>}<br />{themes.find(item => item.id === theme.id)?.name ?? theme.name}</div></div>
  </aside>
  <div className="settings-resizer" role="separator" aria-orientation="vertical" aria-label={t("settings.resizeSidebar")}
    aria-valuemin={150} aria-valuemax={420} aria-valuenow={navWidth === null ? undefined : Math.round(navWidth)} tabIndex={0}
    onPointerDown={beginResize} onKeyDown={resizeWithKeyboard}
    onDoubleClick={() => { setNavWidth(null); try { localStorage.removeItem("rumahl.settings.sidenav-width"); } catch { /* ignore */ } }} />
  <div className="content settings-content" key={pathname}>{content}</div></div>;
}

function SettingsOverview() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { mode } = useShell();
  const { theme } = useTheme();
  return <section className="display-settings">
    <header className="settings-page-heading"><h1 className="pagetitle">{t("nav.settings")}</h1><p className="pagedesc">{t("settings.overviewHelp")}</p></header>
    <div className="settings-group settings-nav">
      <NavRow to="/settings/display" icon={<DesktopIcon />} title={t("settings.personalization")} subtitle={`${t(`mode.${settings.mode}`)} · ${themes.find(item => item.id === theme.id)?.name ?? theme.name}`} />
      <NavRow to="/settings/workspace" icon={<GridIcon />} title={t("workspace.title")} subtitle={t("settings.workspaceHelp")} />
      <NavRow to="/settings/store" icon={<StoreIcon />} title={t("storeSources.title")} subtitle={t("storeSources.overviewHelp")} />
      <NavRow to="/settings/apps" icon={<GridIcon />} title={t("settings.apps")} subtitle={t("settings.appsHelp")} />
      <NavRow to="/settings/accessibility" icon={<PulseIcon />} title={t("settings.accessibility")} subtitle={t("settings.accessibilityHelp")} />
      {mode === "mobile" ? <NavRow to="/settings/mobile" icon={<PhoneIcon />} title={t("settings.mobile")} subtitle={t("mobile.help")} /> : null}
      <NavRow to="/settings/os" icon={<ShieldIcon />} title={t("settings.osMode")} subtitle={t("settings.osModeHelp")} />
      <NavRow to="/settings/device" icon={<DesktopIcon />} title={t("device.title")} subtitle={t("device.help")} />
      <NavRow to="/settings/system" icon={<ShieldIcon />} title={t("settings.system")} subtitle={t("settings.systemHelp")} />
    </div>
  </section>;
}

function AppsSettings() {
  const { t } = useI18n();
  const { dispatch } = useShell();
  const statusIndicators = useAppStatusIndicators();
  const disabled = useDisabledApps();
  const managed = useShellApps({ includeDisabled: true });
  const apps = managed.filter((app) => app.managePath);
  const systemApps = managed.filter((app) => app.system && app.kind === "app");
  return <section className="display-settings">
    <SettingsHeading title={t("settings.apps")} help={t("settings.appsHelp")} back="/settings" />
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("settings.appStatusIndicators")}</strong><p>{t("settings.appStatusIndicatorsHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("settings.appStatusIndicators")} checked={statusIndicators} onChange={(event) => setAppStatusIndicators(event.target.checked)} /></div>
      </div>
    </div>
    {apps.length
      ? <div className="settings-group settings-nav">
          {apps.map((app) => <NavRow key={app.id} to={app.managePath!} title={app.title}
            subtitle={disabled.includes(app.id) ? t("settings.appDisabled") : t("settings.appManage")}
            icon={<span className="settings-nav__app" aria-hidden="true"><AppIcon app={app} /></span>} />)}
        </div>
      : <p className="settings-footnote">{t("settings.appsEmpty")}</p>}
    {systemApps.length ? <>
      <h3 className="section-title">{t("settings.systemApps")}</h3>
      <div className="settings-group">
        {systemApps.map((app) => <div className="setting" key={app.id}>
          <span className="settings-nav__app" aria-hidden="true"><AppIcon app={app} /></span>
          <div className="copy"><strong>{app.title}</strong><p>{disabled.includes(app.id) ? t("settings.appDisabled") : t("settings.appEnabledHelp")}</p></div>
          <div className="visual"><input type="checkbox" role="switch" aria-label={app.title} checked={!disabled.includes(app.id)} onChange={(event) => { const enabled = event.target.checked; setAppEnabled(app.id, enabled); if (!enabled) dispatch({ type: "close-window", id: `app:${app.id}` }); }} /></div>
        </div>)}
      </div>
    </> : null}
  </section>;
}

function AppManagement() {
  const { appId = "" } = useParams();
  const { apps, status } = useAppCatalog();
  const shellApps = useShellApps({ includeDisabled: true });
  const { t } = useI18n();
  const app = shellApps.find((entry) => entry.id === appId && entry.managePath);
  if (status === "loading" && !app) return <p role="status">{t("apps.loading")}</p>;
  if (!app) return <section className="display-settings">
    <SettingsHeading title={t("settings.apps")} help={t("settings.appsHelp")} back="/settings/apps" />
    <p role="alert">{t("settings.appMissing")}</p>
  </section>;
  return <AppManagementBody app={app} details={apps.find((entry) => entry.id === app.id)} />;
}

function AppManagementBody({ app, details }: { app: ShellApp; details?: InstalledApp | undefined }) {
  const { t, locale } = useI18n();
  const { live, open, state, dispatch } = useShell();
  const policy = useOsModePolicy();
  const disabled = useDisabledApps();
  const [ownSettings, setOwnSettings] = useState(false);
  const [runtime, setRuntime] = useState<AppRuntime["state"] | null>(null);
  const [runtimeBusy, setRuntimeBusy] = useState(false);
  const [runtimeError, setRuntimeError] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);
  const [logs, setLogs] = useState<readonly AuditEvent[] | null>(null);
  const [logsError, setLogsError] = useState(false);
  const [logsRevision, setLogsRevision] = useState(0);
  const [logsOffset, setLogsOffset] = useState(0);
  const [logsNext, setLogsNext] = useState<number | null>(null);
  const [logsView, setLogsView] = useState<"simple" | "detailed">("simple");
  const [sensors, setSensors] = useState<Sensors | null>(null);
  const [sensorsError, setSensorsError] = useState(false);
  const [containerOptions, setContainerOptions] = useState({ startOnBoot: false, watchdog: false, autoUpdate: false });
  const [confirmUninstall, setConfirmUninstall] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);
  const [uninstallError, setUninstallError] = useState(false);
  const isContainer = details?.runtime === "container";
  const lifecycle = details?.lifecycle ?? "on-demand";
  const runtimeKind = details?.runtime;
  const typeLabel = runtimeKind === "container"
    ? t("settings.appType.container")
    : runtimeKind === "web" || runtimeKind === "native"
      ? t("settings.appType.native")
      : t("settings.appType.unknown");
  const nativeRunning = state.windows.some((item) => !item.minimized && (item.location === app.path || item.location?.startsWith(`${app.path}/`)));

  useEffect(() => {
    if (!live || !details) { setOwnSettings(false); return; }
    const controller = new AbortController();
    fetchAppSettings(live.request, details, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setOwnSettings(value.manifest.length > 0); })
      .catch(() => { if (!controller.signal.aborted) setOwnSettings(false); });
    return () => controller.abort();
  }, [live, details]);

  useEffect(() => {
    if (!live || !details || !isContainer) { setRuntime(null); return; }
    const controller = new AbortController();
    fetchAppRuntime(live.request, details, controller.signal)
      .then((value) => { if (!controller.signal.aborted) setRuntime(value.state); })
      .catch(() => { if (!controller.signal.aborted) setRuntime(null); });
    return () => controller.abort();
  }, [live, details, isContainer]);

  useEffect(() => {
    if (!live || !details || !isContainer) { setSensors(null); setSensorsError(false); return; }
    const controller = new AbortController();
    const load = () => {
      fetchSensors(live.request)
        .then((value) => { if (!controller.signal.aborted) { setSensors(value); setSensorsError(false); } })
        .catch(() => { if (!controller.signal.aborted) { setSensors(null); setSensorsError(true); } });
    };
    load();
    const timer = window.setInterval(load, 5000);
    return () => { controller.abort(); window.clearInterval(timer); };
  }, [live, details, isContainer]);

  useEffect(() => {
    if (!isContainer) return;
    const key = `rumahl.app.container.${app.id}`;
    let stored: Partial<typeof containerOptions>;
    try { stored = JSON.parse(localStorage.getItem(key) ?? "null") ?? {}; } catch { stored = {}; }
    setContainerOptions({
      startOnBoot: details?.lifecycle === "always-on",
      watchdog: false,
      autoUpdate: false,
      ...stored
    });
  }, [app.id, isContainer, details?.lifecycle]);

  useEffect(() => {
    if (!logsOpen || !live || !details) return;
    const controller = new AbortController();
    const load = () => {
      fetchAudit(live.request, { target: details.id, limit: 20, offset: logsOffset })
        .then((value) => { if (!controller.signal.aborted) { setLogs(value.events); setLogsNext(value.nextOffset); setLogsError(false); } })
        .catch(() => { if (!controller.signal.aborted) { setLogs([]); setLogsNext(null); setLogsError(true); } });
    };
    load();
    const timer = logsOffset === 0 ? window.setInterval(load, 5000) : undefined;
    return () => { controller.abort(); if (timer !== undefined) window.clearInterval(timer); };
  }, [logsOpen, live, details, logsRevision, logsOffset]);

  useEffect(() => {
    if (!logsOpen) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") setLogsOpen(false); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [logsOpen]);

  async function runRuntime(action: "start" | "stop" | "restart") {
    if (!live || !details || !isContainer || runtimeBusy) return;
    setRuntimeBusy(true);
    setRuntimeError(false);
    const controller = new AbortController();
    try {
      if (action === "restart") {
        const response = await restartComponent(live.request, "app", { appId: details.id, installationId: details.installationId });
        if (!response.ok) throw new Error("restart failed");
        setRuntime("running");
      } else {
        const value = await controlAppRuntime(live.request, details, action, controller.signal);
        setRuntime(value.state);
      }
    } catch {
      setRuntimeError(true);
    } finally {
      setRuntimeBusy(false);
    }
  }

  async function uninstall() {
    if (!live || !details || uninstalling) return;
    setUninstalling(true);
    setUninstallError(false);
    const controller = new AbortController();
    try {
      await uninstallApp(live.request, details, controller.signal);
      window.dispatchEvent(new Event(CATALOG_REFRESH_EVENT));
      open("/settings/apps");
    } catch {
      setUninstallError(true);
    } finally {
      setUninstalling(false);
    }
  }

  function setContainerOption(key: keyof typeof containerOptions, value: boolean) {
    const next = { ...containerOptions, [key]: value };
    setContainerOptions(next);
    try { localStorage.setItem(`rumahl.app.container.${app.id}`, JSON.stringify(next)); } catch { /* storage remains in memory */ }
  }

  function openLogs() {
    setLogs(null);
    setLogsError(false);
    setLogsOffset(0);
    setLogsNext(null);
    setLogsOpen(true);
  }

  const capabilities = details?.capabilities ?? [];
  return <>
    <section className="display-settings app-management">
      <SettingsHeading title={app.title} help={t("settings.appManageHelp")} back="/settings/apps" />
      <div className="settings-group">
        <div className="setting app-management__identity">
          <span className="app-management__icon" aria-hidden="true"><AppIcon app={app} /></span>
          <div className="copy"><strong>{app.title}</strong><p>{app.id}{details ? ` · ${t("settings.appVersion")} ${details.version}` : ""}</p></div>
          <div className="visual"><Button size="sm" variant="primary" disabled={!app.launchable} onClick={() => open(app.path)}>{t("settings.appOpen")}</Button></div>
        </div>
        {details ? <div className="setting setting--value">
          <div className="copy"><strong>{t("settings.appType")}</strong></div>
          <div className="visual"><span className="app-management__value">{typeLabel}</span></div>
        </div> : null}
        {details ? <div className="setting">
          <div className="copy"><strong>{t("settings.appEnabled")}</strong><p>{t("settings.appEnabledHelp")}</p></div>
          <div className="visual"><input type="checkbox" role="switch" aria-label={t("settings.appEnabled")} checked={!disabled.includes(app.id)} onChange={(event) => {
            const enabled = event.target.checked;
            setAppEnabled(app.id, enabled);
            if (!enabled) {
              dispatch({ type: "close-window", id: `app:${app.id}` });
              if (isContainer && live && details) void controlAppRuntime(live.request, { id: details.id, installationId: details.installationId }, "stop", new AbortController().signal).catch(() => undefined);
            }
          }} /></div>
        </div> : null}
        {details ? <div className="setting setting--value">
          <div className="copy"><strong>{t("settings.appStartBehavior")}</strong><p>{t(`settings.appStartBehavior.${lifecycle}Help`)}</p></div>
          <div className="visual"><span className="app-management__value">{t(`settings.appStartBehavior.${lifecycle}`)}</span></div>
        </div> : null}
        {details && !isContainer && runtimeKind ? <div className="setting setting--value">
          <div className="copy"><strong>{t("settings.appStatus")}</strong></div>
          <div className="visual"><span className={`app-management__status app-management__status--${nativeRunning ? "running" : "stopped"}`}><i aria-hidden="true" />{nativeRunning ? t("settings.appRuntime.running") : t("settings.appRuntime.closed")}</span></div>
        </div> : null}
        {policy.advancedSettings && details ? <div className="setting setting--value">
          <div className="copy"><strong>{t("settings.appInstallation")}</strong></div>
          <div className="visual"><code>{details.installationId}</code></div>
        </div> : null}
      </div>

      {details ? <>
        <h3 className="section-title">{t("settings.appPermissions")}</h3>
        <div className="settings-group">
          {capabilities.length
            ? capabilities.map((capability) => <div className="setting setting--value" key={capability}>
                <div className="copy"><strong>{permissionLabel(capability)}</strong><p>{capability}</p></div>
                <div className="visual"><span className="app-management__granted">{t("settings.appGranted")}</span></div>
              </div>)
            : <div className="setting">
                <div className="copy"><strong>{t("settings.appNoPermissions")}</strong><p>{t("settings.appNoPermissionsHelp")}</p></div>
              </div>}
        </div>
        <p className="settings-footnote">{t("settings.appPermissionsHelp")}</p>

        {isContainer ? <>
          <h3 className="section-title">{t("settings.appRuntime")}</h3>
          <div className="settings-group">
            <div className="setting">
              <div className="copy">
                <strong>{t("settings.appRuntime")}</strong>
                <p><span className={`app-management__status app-management__status--${runtime ?? "unknown"}`}><i aria-hidden="true" />{runtime ? t(`settings.appRuntime.${runtime}`) : t("settings.appRuntime.unavailable")}</span></p>
              </div>
              <div className="visual app-management__actions">
                <Button size="sm" variant="primary" backgroundColor="#2f9e68" leftIcon={<PlayIcon />} disabled={runtimeBusy || runtime === "running"} onClick={() => void runRuntime("start")}>{t("settings.appStart")}</Button>
                <Button size="sm" variant="danger" leftIcon={<StopIcon />} disabled={runtimeBusy || runtime !== "running"} onClick={() => void runRuntime("stop")}>{t("settings.appStop")}</Button>
                <Button size="sm" variant="danger" leftIcon={<RefreshIcon />} disabled={runtimeBusy || runtime !== "running"} onClick={() => void runRuntime("restart")}>{t("settings.appRestart")}</Button>
              </div>
            </div>
            {runtimeError ? <p className="settings-footnote" role="alert">{t("settings.appRuntimeFailed")}</p> : null}
          </div>
        </> : null}

        {isContainer ? <>
          <h3 className="section-title">{t("settings.appContainerOptions")}</h3>
          <div className="settings-group">
            <div className="setting">
              <div className="copy"><strong>{t("settings.appStartOnBoot")}</strong><p>{t("settings.appStartOnBootHelp")}</p></div>
              <div className="visual"><input type="checkbox" role="switch" aria-label={t("settings.appStartOnBoot")} checked={containerOptions.startOnBoot} onChange={(event) => setContainerOption("startOnBoot", event.target.checked)} /></div>
            </div>
            <div className="setting">
              <div className="copy"><strong>{t("settings.appWatchdog")}</strong><p>{t("settings.appWatchdogHelp")}</p></div>
              <div className="visual"><input type="checkbox" role="switch" aria-label={t("settings.appWatchdog")} checked={containerOptions.watchdog} onChange={(event) => setContainerOption("watchdog", event.target.checked)} /></div>
            </div>
            <div className="setting">
              <div className="copy"><strong>{t("settings.appAutoUpdate")}</strong><p>{t("settings.appAutoUpdateHelp")}</p></div>
              <div className="visual"><input type="checkbox" role="switch" aria-label={t("settings.appAutoUpdate")} checked={containerOptions.autoUpdate} onChange={(event) => setContainerOption("autoUpdate", event.target.checked)} /></div>
            </div>
          </div>

          <h3 className="section-title">{t("settings.appResources")}</h3>
          <div className="settings-group">
            {sensors ? <>
              <div className="setting setting--value"><div className="copy"><strong>{t("settings.appHostname")}</strong></div><div className="visual"><code>{sensors.host ?? "—"}</code></div></div>
              {sensors.metrics.map((metric) => <div className="setting setting--value" key={metric.id}>
                <div className="copy"><strong>{metric.id === "cpu" ? t("settings.appMetric.cpu") : metric.id === "memory" ? t("settings.appMetric.memory") : metric.id === "temperature" ? t("settings.appMetric.temperature") : metric.label}</strong></div>
                <div className="visual"><span className="app-management__value">{metric.value}{metric.unit}</span></div>
              </div>)}
              {sensors.uptime !== null ? <div className="setting setting--value"><div className="copy"><strong>{t("settings.appUptime")}</strong></div><div className="visual"><span className="app-management__value">{formatUptime(sensors.uptime)}</span></div></div> : null}
            </> : <div className="setting"><div className="copy"><strong>{sensorsError ? t("settings.appResourcesUnavailable") : t("settings.appResourcesLoading")}</strong></div></div>}
          </div>
        </> : null}

        <h3 className="section-title">{t("settings.appLogs")}</h3>
        <div className="settings-group">
          <div className="setting">
            <div className="copy"><strong>{t("settings.appLogs")}</strong><p>{t("settings.appLogsHelp")}</p></div>
            <div className="visual"><Button size="sm" onClick={openLogs}>{t("settings.appOpenLogs")}</Button></div>
          </div>
        </div>

        {ownSettings ? <>
          <h3 className="section-title">{t("settings.appOwnSection")}</h3>
          <div className="settings-group settings-nav">
            <NavRow to={appPath(app.id, ["settings"])} icon={<SettingsIcon />} title={t("appSettings.open")} subtitle={t("settings.appOwnSettingsHelp")} />
          </div>
        </> : null}

        <h3 className="section-title">{t("settings.appDanger")}</h3>
        <div className="settings-group">
          {confirmUninstall
            ? <div className="setting">
                <div className="copy"><strong>{t("settings.appUninstallConfirm")}</strong><p>{t("settings.appUninstallWarning")}</p></div>
                <div className="visual app-management__actions">
                  <Button size="sm" disabled={uninstalling} onClick={() => setConfirmUninstall(false)}>{t("settings.appCancel")}</Button>
                  <Button size="sm" variant="danger" disabled={uninstalling} onClick={() => void uninstall()}>{uninstalling ? t("settings.appUninstalling") : t("settings.appUninstall")}</Button>
                </div>
              </div>
            : <div className="setting">
                <div className="copy"><strong>{t("settings.appUninstall")}</strong><p>{t("settings.appUninstallHelp")}</p></div>
                <div className="visual"><Button size="sm" variant="danger" onClick={() => setConfirmUninstall(true)}>{t("settings.appUninstall")}</Button></div>
              </div>}
          {uninstallError ? <p className="settings-footnote" role="alert">{t("settings.appUninstallFailed")}</p> : null}
        </div>
      </> : null}
    </section>

    {logsOpen && details ? createPortal(
      <div className="app-management-modal" role="dialog" aria-modal="true" aria-labelledby="app-logs-title" onClick={(event) => { if (event.target === event.currentTarget) setLogsOpen(false); }}>
        <div className="app-management-modal__surface">
          <header className="app-management-modal__head">
            <div className="app-management-modal__title">
              <p className="app-management-modal__eyebrow">{app.title}</p>
              <h2 id="app-logs-title">{t("settings.appLogs")}</h2>
            </div>
            <RumahlButtonGroup<"simple" | "detailed">
              label={t("settings.appLogsView")}
              size="sm"
              value={logsView}
              onChange={setLogsView}
              options={[
                { value: "simple", label: t("settings.appLogsSimple") },
                { value: "detailed", label: t("settings.appLogsDetailed") }
              ]}
            />
            {logsOffset === 0 ? <span className="app-management__live"><i aria-hidden="true" />{t("settings.appLogsLive")}</span> : null}
            <button type="button" className="app-management-modal__close" aria-label={t("settings.appClose")} onClick={() => setLogsOpen(false)}>×</button>
          </header>
          <div className="app-management-modal__body" role="log" aria-live="polite">
            {logs === null
              ? <p className="settings-footnote" role="status">{t("settings.appLogsLoading")}</p>
              : logs.length === 0
                ? <p className="settings-footnote">{logsError ? t("settings.appLogsUnavailable") : t("settings.appLogsEmpty")}</p>
                : logs.map((event) => logsView === "simple"
                  ? <div className="app-management__log" key={event.sequence}>
                      <div>
                        <strong>{event.action.replaceAll("-", " ")}</strong>
                        <p>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" }).format(new Date(event.at * 1000))}</p>
                      </div>
                      <span className={`app-management__outcome app-management__outcome--${event.outcome}`}>{event.outcome}</span>
                    </div>
                  : <article className="app-management__log app-management__log--detailed" key={event.sequence}>
                      <div className="app-management__log-head">
                        <strong>{event.action.replaceAll("-", " ")}</strong>
                        <span className={`app-management__outcome app-management__outcome--${event.outcome}`}>{event.outcome}</span>
                      </div>
                      <dl className="app-management__log-grid">
                        <div><dt>{t("settings.appLogsTime")}</dt><dd>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" }).format(new Date(event.at * 1000))}</dd></div>
                        <div><dt>{t("settings.appLogsActor")}</dt><dd>{event.actor || "—"}</dd></div>
                        <div><dt>{t("settings.appLogsTarget")}</dt><dd>{event.target ?? "—"}</dd></div>
                        <div><dt>{t("settings.appLogsSequence")}</dt><dd>{event.sequence}</dd></div>
                      </dl>
                    </article>)}
          </div>
          <footer className="app-management-modal__foot">
            <div className="app-management-modal__pager">
              <Button size="sm" disabled={logsOffset === 0} onClick={() => { setLogs(null); setLogsOffset(Math.max(0, logsOffset - 20)); }}>{t("settings.appLogsNewer")}</Button>
              <span>{t("settings.appLogsPage", { page: Math.floor(logsOffset / 20) + 1 })}</span>
              <Button size="sm" disabled={logsNext === null} onClick={() => { setLogs(null); setLogsOffset(logsNext!); }}>{t("settings.appLogsOlder")}</Button>
            </div>
            <div className="app-management-modal__actions">
              <Button size="sm" onClick={() => setLogsRevision((value) => value + 1)}>{t("settings.appLogsRefresh")}</Button>
              <Button size="sm" onClick={() => setLogsOpen(false)}>{t("settings.appClose")}</Button>
            </div>
          </footer>
        </div>
      </div>,
      document.querySelector(".shell") ?? document.body
    ) : null}
  </>;
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function permissionLabel(capability: string): string {
  const tail = capability.replace(/^com\.rumahl\.os\./, "").replace(/[._-]+/g, " ").trim();
  return tail ? tail[0]!.toLocaleUpperCase() + tail.slice(1) : capability;
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
  const { snapshot, mode, live } = useShell();
  const { theme } = useTheme();
  return <section className="display-settings">
    <SettingsHeading title={t("settings.system.about")} help={t("settings.system.aboutHelp")} back="/settings/system" />

    <div className="settings-group">
      <div className="setting about-identity">
        <span className="about-identity__mark" aria-hidden="true"><RumahlMark /></span>
        <div className="copy"><strong>{osName(Boolean(live?.demo))}</strong><p>{t("settings.system.tagline")}</p></div>
      </div>
    </div>

    <h3 className="section-title">{t("settings.system.session")}</h3>
    <div className="settings-group">
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.user")}</strong></div><div className="visual"><span>{snapshot.user.displayName}</span></div></div>
      <div className="setting setting--value"><div className="copy"><strong>{t("settings.system.version")}</strong></div><div className="visual"><code>{versionLabel(Boolean(live?.demo))}</code></div></div>
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
