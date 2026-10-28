import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";
import { useAppCatalog } from "../apps/AppCatalog";
import { useOsMode } from "../preferences/OsMode";
import { useClock } from "./clock";
import { closeEmergency } from "./emergency";
import { EventMonitor } from "./EventMonitor";
import type { MessageKey } from "../i18n/locales/en";
import {
  fetchComponents,
  fetchRecoveryStatus,
  fetchSensors,
  restartComponent,
  runSelfHeal,
  type RecoveryComponent,
  type RecoveryStatus,
  type Sensors
} from "../apps/recovery-client";
import "./emergency.css";

type Section = "status" | "activity" | "components" | "sensors" | "recovery";
interface Pending { title: string; body: string; confirm: string; run: () => Promise<void> | void }

const CHECK_LABEL: Record<string, MessageKey> = {
  platform: "emergency.check.platform",
  renderer: "emergency.check.renderer",
  audit: "emergency.check.audit",
  apps: "emergency.check.apps",
  sensors: "emergency.check.sensors"
};

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  return days > 0 ? `${days}d ${hours}h` : hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}
function formatClock(date: Date | null): string {
  return date ? new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(date) : "";
}

/**
 * The Emergency & Recovery Center: a self-contained minimal OS, not an overlay.
 * It has its own system bar, sidebar navigation and status bar, is fully guided,
 * and adapts its status wording to the operating-system mode.
 */
export function EmergencyShell() {
  const { t, locale } = useI18n();
  const { live, snapshot } = useShell();
  const { apps } = useAppCatalog();
  const { mode } = useOsMode();
  const now = useClock();
  const [section, setSection] = useState<Section>("status");
  const [status, setStatus] = useState<RecoveryStatus | null>(null);
  const [components, setComponents] = useState<RecoveryComponent[]>([]);
  const [sensors, setSensors] = useState<Sensors | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const request = live?.request;
  const detailed = mode !== "guided";

  const loadStatus = useCallback(() => {
    if (!request) return;
    void fetchRecoveryStatus(request).then(setStatus).catch(() => setStatus(null));
  }, [request]);
  useEffect(() => { loadStatus(); }, [loadStatus]);
  useEffect(() => { if (request) void fetchSensors(request).then(setSensors).catch(() => undefined); }, [request]);

  useEffect(() => {
    if (!request || section !== "components") return;
    void fetchComponents(request).then((value) => setComponents(value.components)).catch(() => setComponents([]));
  }, [request, section]);

  useEffect(() => {
    if (!request || section !== "sensors") return;
    let active = true;
    const refresh = () => void fetchSensors(request).then((value) => { if (active) setSensors(value); }).catch(() => undefined);
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => { active = false; clearInterval(timer); };
  }, [request, section]);

  const installed = apps.filter((app) => app.launchable);
  function ask(action: Pending) { setPending(action); }
  async function confirm() {
    if (!pending) return;
    const action = pending;
    setPending(null);
    setBusy(true);
    try { await action.run(); } finally { setBusy(false); }
  }
  const reloadShell = () => window.location.reload();
  const restartApp = (appId: string, installationId: string) => {
    if (!request) return;
    ask({ title: t("emergency.restartApp"), body: t("emergency.restartAppBody", { name: appId }), confirm: t("emergency.confirm"), run: () => restartComponent(request, "app", { appId, installationId }).then(() => undefined) });
  };
  const restartAllApps = () => {
    if (!request || installed.length === 0) return;
    ask({ title: t("emergency.restartAllApps"), body: t("emergency.restartAllAppsBody", { count: installed.length }), confirm: t("emergency.confirm"), run: async () => { for (const app of installed) await restartComponent(request, "app", { appId: app.id, installationId: app.installationId }); } });
  };
  const selfHeal = () => {
    if (!request) return;
    ask({ title: t("emergency.runSelfHeal"), body: t("emergency.runSelfHealBody"), confirm: t("emergency.confirm"), run: async () => { await runSelfHeal(request); loadStatus(); } });
  };

  const navItem = (id: Section, label: string) =>
    <button type="button" role="tab" aria-selected={section === id} className={`emergency__navitem${section === id ? " is-active" : ""}`} onClick={() => setSection(id)}>{label}</button>;

  const okText = t(detailed ? "emergency.statusOkAdvanced" : "emergency.statusOkGuided");
  const badText = t(detailed ? "emergency.statusAttentionAdvanced" : "emergency.statusAttentionGuided");
  const sensorLevel = sensors?.level ?? "information";

  return createPortal(<div className="emergency" role="dialog" aria-modal="true" aria-label={t("emergency.title")}>
    <header className="emergency__sysbar">
      <span className="emergency__brand"><span className="emergency__dot" />rumahl OS</span>
      <span className="emergency__chip">{t("emergency.title")}</span>
      <div className="emergency__sysright">
        {now ? <time className="emergency__clock">{formatClock(now)}</time> : null}
        <span className="emergency__user">{snapshot.user.displayName}</span>
        <button type="button" className="btn btn--primary" onClick={closeEmergency}>{t("emergency.resume")}</button>
      </div>
    </header>
    <div className="emergency__frame">
      <nav className="emergency__nav" role="tablist" aria-label={t("emergency.title")}>
        {navItem("status", t("emergency.tab.status"))}
        {navItem("activity", t("emergency.tab.activity"))}
        {navItem("components", t("emergency.tab.components"))}
        {navItem("sensors", t("emergency.tab.sensors"))}
        {navItem("recovery", t("emergency.tab.recovery"))}
      </nav>
      <main className="emergency__main">
        {section === "status" ? <section className="emergency__panel">
          <h2>{t("emergency.statusHeading")}</h2>
          {status ? <>
            <div className={`emergency__verdict is-${status.level}`}>
              <span className="emergency__verdictDot" aria-hidden="true" />
              <div>
                <strong>{status.ok ? okText : badText}</strong>
                <p>{status.ok ? t("emergency.statusOkHint") : t("emergency.statusAttentionHint")}</p>
              </div>
            </div>
            {detailed ? <ul className="emergency__list emergency__checks">
              {status.checks.map((check) => <li key={check.id} className="emergency__row">
                <span className={`emergency__state is-${check.state}`} aria-hidden="true" />
                <span className="emergency__name">{t(CHECK_LABEL[check.id] ?? "emergency.check.platform")}</span>
                <span className="emergency__stateText">{check.state}</span>
              </li>)}
            </ul> : null}
            <div className="emergency__actions">
              <button type="button" className="btn" disabled={busy} onClick={selfHeal}>{t("emergency.runSelfHeal")}</button>
              <button type="button" className="btn" onClick={loadStatus}>{t("recovery.refresh")}</button>
            </div>
          </> : <p className="emergency__muted">{t("recovery.unavailable")}</p>}
        </section> : null}

        {section === "activity" ? <section className="emergency__panel">
          <h2>{t("emergency.activityHeading")}</h2>
          {request ? <EventMonitor request={request} /> : <p className="emergency__muted">{t("recovery.unavailable")}</p>}
        </section> : null}

        {section === "components" ? <section className="emergency__panel">
          <h2>{t("emergency.componentsHeading")}</h2>
          <ul className="emergency__list">
            {components.map((component) => <li key={component.id} className="emergency__row">
              <span className={`emergency__state is-${component.state}`} aria-hidden="true" />
              <span className="emergency__name">{component.id}</span>
              <span className="emergency__kind">{component.kind}</span>
              <span className="emergency__stateText">{component.state}</span>
              {component.id === "shell" ? <button type="button" className="btn" disabled={busy} onClick={() => ask({ title: t("emergency.restartShell"), body: t("emergency.restartShellBody"), confirm: t("emergency.confirm"), run: reloadShell })}>{t("emergency.restart")}</button> : null}
            </li>)}
          </ul>
          <h3>{t("emergency.appsHeading")}</h3>
          <div className="emergency__actions">
            <button type="button" className="btn" disabled={busy || installed.length === 0} onClick={restartAllApps}>{t("emergency.restartAllApps")}</button>
          </div>
          <ul className="emergency__list">
            {installed.length === 0 ? <li className="emergency__muted">{t("emergency.noApps")}</li> : installed.map((app) => <li key={app.id} className="emergency__row">
              <span className="emergency__state is-running" aria-hidden="true" />
              <span className="emergency__name">{app.title}</span>
              <span className="emergency__kind">{app.id}</span>
              <span className="emergency__stateText">{t("emergency.installed")}</span>
              <button type="button" className="btn" disabled={busy} onClick={() => restartApp(app.id, app.installationId)}>{t("emergency.restart")}</button>
            </li>)}
          </ul>
        </section> : null}

        {section === "sensors" ? <section className="emergency__panel">
          <h2>{t("emergency.sensorsHeading")}</h2>
          {sensors ? <>
            <div className="emergency__sensorsMeta">
              <span>{t("emergency.host")}: <b>{sensors.host ?? "—"}</b></span>
              <span>{t("emergency.cores")}: <b>{sensors.cores}</b></span>
              <span>{t("emergency.uptime")}: <b>{sensors.uptime != null ? formatUptime(sensors.uptime) : "—"}</b></span>
            </div>
            <div className="emergency__sensors">
              {sensors.metrics.map((metric) => <div key={metric.id} className={`emergency__sensor is-${metric.level}`}>
                <span className="emergency__sensorLabel">{metric.label}</span>
                <span className="emergency__sensorValue">{metric.value}<small>{metric.unit}</small></span>
              </div>)}
              {sensors.metrics.length === 0 ? <p className="emergency__muted">{t("emergency.noSensors")}</p> : null}
            </div>
          </> : <p className="emergency__muted">{t("recovery.unavailable")}</p>}
        </section> : null}

        {section === "recovery" ? <section className="emergency__panel">
          <h2>{t("emergency.recoveryHeading")}</h2>
          <p className="emergency__muted">{t("emergency.recoveryHint")}</p>
          <div className="emergency__actions">
            <button type="button" className="btn" disabled={busy} onClick={selfHeal}>{t("emergency.runSelfHeal")}</button>
            <button type="button" className="btn" disabled={busy} onClick={() => ask({ title: t("emergency.restartShell"), body: t("emergency.restartShellBody"), confirm: t("emergency.confirm"), run: reloadShell })}>{t("emergency.restartShell")}</button>
            <a className="btn" href="/logout">{t("emergency.signOut")}</a>
            <a className="btn" href="/recovery">{t("emergency.openRecovery")}</a>
          </div>
        </section> : null}
      </main>
    </div>
    <footer className="emergency__statusbar">
      <span>{t("emergency.host")}: {sensors?.host ?? "—"}</span>
      <span>{t("emergency.uptime")}: {sensors?.uptime != null ? formatUptime(sensors.uptime) : "—"}</span>
      <span className={`emergency__barlevel is-${sensorLevel}`}>{t("emergency.tab.sensors")}: {sensorLevel}</span>
      <span className="emergency__barlast">{locale.toUpperCase()} · #{status?.lastAuditSequence ?? 0}</span>
    </footer>

    {pending ? <div className="emergency__confirm" role="alertdialog" aria-modal="true">
      <div className="emergency__confirmBox">
        <h2>{pending.title}</h2>
        <p>{pending.body}</p>
        <div className="emergency__actions">
          <button type="button" className="btn btn--primary" disabled={busy} onClick={() => void confirm()}>{pending.confirm}</button>
          <button type="button" className="btn" disabled={busy} onClick={() => setPending(null)}>{t("emergency.cancel")}</button>
        </div>
      </div>
    </div> : null}
  </div>, document.body);
}
