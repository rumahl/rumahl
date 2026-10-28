import { useCallback, useEffect, useState } from "react";
import { useI18n } from "../i18n";
import type { ShellRequest } from "../snapshot-client";
import { AUDIT_ACTIONS, fetchAudit, type AuditEvent } from "../apps/recovery-client";

/** Windows-style severity derived from the audit outcome. */
function levelOf(outcome: AuditEvent["outcome"]): "information" | "warning" | "error" {
  return outcome === "failure" ? "error" : outcome === "denied" ? "warning" : "information";
}

/**
 * Human-readable source. The OS layer is always surfaced as "rumahl OS" — an
 * internal layer name is never shown.
 */
function sourceOf(actor: string): string {
  if (actor === "os") return "rumahl OS";
  if (actor === "system") return "System";
  if (actor.startsWith("user:")) return "User";
  if (actor.startsWith("service:")) return actor.slice("service:".length);
  if (actor.startsWith("app:")) return actor.slice("app:".length);
  return actor;
}

function formatTime(seconds: number): string {
  const date = new Date(seconds * 1000);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}

/** The event monitor: the central activity log, in a Windows-style viewer. */
export function EventMonitor({ request }: { request: ShellRequest }) {
  const { t } = useI18n();
  const [level, setLevel] = useState("");
  const [action, setAction] = useState("");
  const [actor, setActor] = useState("");
  const [target, setTarget] = useState("");
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [offset, setOffset] = useState(0);
  const [next, setNext] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [selected, setSelected] = useState<AuditEvent | null>(null);

  const load = useCallback((nextOffset: number) => {
    setLoading(true);
    setError(false);
    void fetchAudit(request, {
      action: action || undefined,
      outcome: level || undefined,
      actor: actor.trim() || undefined,
      target: target.trim() || undefined,
      limit: 100,
      offset: nextOffset
    })
      .then((page) => { setEvents(page.events); setNext(page.nextOffset); setOffset(nextOffset); })
      .catch(() => setError(true))
      .finally(() => setLoading(false));
  }, [request, level, action, actor, target]);

  useEffect(() => { load(0); }, [load]);

  const label = (value: string) => value.replace(/-/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());

  return <div className="event-monitor">
    <form className="event-monitor__filters" onSubmit={(event) => { event.preventDefault(); load(0); }}>
      <label>{t("recovery.filter.level")}
        <select value={level} onChange={(event) => setLevel(event.target.value)}>
          <option value="">{t("recovery.filter.any")}</option>
          <option value="success">{t("recovery.level.information")}</option>
          <option value="denied">{t("recovery.level.warning")}</option>
          <option value="failure">{t("recovery.level.error")}</option>
        </select>
      </label>
      <label>{t("recovery.filter.action")}
        <select value={action} onChange={(event) => setAction(event.target.value)}>
          <option value="">{t("recovery.filter.any")}</option>
          {AUDIT_ACTIONS.map((value) => <option key={value} value={value}>{label(value)}</option>)}
        </select>
      </label>
      <label>{t("recovery.filter.actor")}
        <input value={actor} onChange={(event) => setActor(event.target.value)} placeholder={t("recovery.filter.actorHint")} />
      </label>
      <label>{t("recovery.filter.target")}
        <input value={target} onChange={(event) => setTarget(event.target.value)} placeholder={t("recovery.filter.targetHint")} />
      </label>
      <button type="submit" className="btn btn--primary">{t("recovery.filter.apply")}</button>
    </form>
    {error ? <p role="alert" className="event-monitor__empty">{t("recovery.unavailable")}</p>
      : events.length === 0 && !loading ? <p className="event-monitor__empty">{t("recovery.noEvents")}</p>
      : <div className="event-monitor__scroll">
        <table className="event-monitor__table">
          <thead><tr>
            <th>{t("recovery.col.level")}</th><th>{t("recovery.col.time")}</th><th>{t("recovery.col.source")}</th>
            <th>{t("recovery.col.action")}</th><th>{t("recovery.col.target")}</th>
          </tr></thead>
          <tbody>
            {events.map((event) => {
              const levelName = levelOf(event.outcome);
              return <tr key={event.sequence} data-level={levelName} className={selected?.sequence === event.sequence ? "is-selected" : ""} onClick={() => setSelected(event)}>
                <td className={`event-monitor__level is-${levelName}`}>{t(`recovery.level.${levelName}`)}</td>
                <td className="event-monitor__time">{formatTime(event.at)}</td>
                <td className="event-monitor__source" title={event.actor}>{sourceOf(event.actor)}</td>
                <td className="event-monitor__action">{label(event.action)}</td>
                <td className="event-monitor__target" title={event.target ?? ""}>{event.target ?? "—"}</td>
              </tr>;
            })}
          </tbody>
        </table>
      </div>}
    {selected ? <div className="event-monitor__details">
      <div className="event-monitor__detailsHead">
        <strong>{t("recovery.details")}</strong>
        <button type="button" className="btn" onClick={() => setSelected(null)}>{t("emergency.close")}</button>
      </div>
      <dl>
        <dt>#</dt><dd>{selected.sequence}</dd>
        <dt>{t("recovery.col.time")}</dt><dd>{formatTime(selected.at)}</dd>
        <dt>{t("recovery.col.level")}</dt><dd className={`is-${levelOf(selected.outcome)}`}>{t(`recovery.level.${levelOf(selected.outcome)}`)}</dd>
        <dt>{t("recovery.col.source")}</dt><dd>{sourceOf(selected.actor)}</dd>
        <dt>{t("recovery.filter.actor")}</dt><dd>{selected.actor}</dd>
        <dt>{t("recovery.col.action")}</dt><dd>{label(selected.action)} ({selected.action})</dd>
        <dt>{t("recovery.col.target")}</dt><dd>{selected.target ?? "—"}</dd>
      </dl>
    </div> : null}
    <div className="event-monitor__pager">
      <button type="button" className="btn" disabled={offset === 0 || loading} onClick={() => load(Math.max(0, offset - 100))}>{t("recovery.newer")}</button>
      <button type="button" className="btn" disabled={next === null || loading} onClick={() => load(next ?? offset)}>{t("recovery.older")}</button>
      <button type="button" className="btn" onClick={() => load(offset)} disabled={loading}>{t("recovery.refresh")}</button>
      <button type="button" className="btn" disabled={events.length === 0} onClick={() => {
        const blob = new Blob([JSON.stringify(events, null, 2)], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `rumahl-activity-${Date.now()}.json`;
        link.click();
        URL.revokeObjectURL(url);
      }}>{t("emergency.exportActivity")}</button>
    </div>
  </div>;
}
