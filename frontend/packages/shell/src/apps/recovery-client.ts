import type { ShellRequest } from "../snapshot-client";

/** Typed access to the recovery / audit API (see platform-web/src/server.rs). */
export interface AuditEvent {
  sequence: number;
  at: number;
  actor: string;
  action: string;
  outcome: "success" | "denied" | "failure";
  target: string | null;
}
export interface AuditPage {
  events: AuditEvent[];
  nextOffset: number | null;
}
export interface AuditFilter {
  action?: string | undefined;
  actor?: string | undefined;
  outcome?: string | undefined;
  target?: string | undefined;
  since?: number | undefined;
  limit?: number | undefined;
  offset?: number | undefined;
}
export interface RecoveryComponent {
  id: string;
  kind: string;
  state: string;
  restartable: boolean;
}
export interface RecoveryStatus {
  ok: boolean;
  level: "information" | "warning" | "error";
  now: number;
  lastAuditSequence: number;
  checks: { id: string; state: string }[];
}
export interface SensorMetric {
  id: string;
  label: string;
  value: number;
  unit: string;
  level: "information" | "warning" | "error";
}
export interface Sensors {
  sensorsVersion: number;
  host: string | null;
  uptime: number | null;
  cores: number;
  level: "information" | "warning" | "error";
  at: number;
  metrics: SensorMetric[];
}

async function getJson<T>(request: ShellRequest, path: string): Promise<T> {
  const response = await request(path, { credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new Error(`recovery ${response.status}`);
  return (await response.json()) as T;
}

export function fetchAudit(request: ShellRequest, filter: AuditFilter): Promise<AuditPage> {
  const query = new URLSearchParams();
  if (filter.action) query.set("action", filter.action);
  if (filter.actor) query.set("actor", filter.actor);
  if (filter.outcome) query.set("outcome", filter.outcome);
  if (filter.target) query.set("target", filter.target);
  if (filter.since) query.set("since", String(filter.since));
  query.set("limit", String(filter.limit ?? 100));
  query.set("offset", String(filter.offset ?? 0));
  return getJson(request, `/api/v1/recovery/audit?${query.toString()}`);
}

export function fetchRecoveryStatus(request: ShellRequest): Promise<RecoveryStatus> {
  return getJson(request, "/api/v1/recovery/status");
}

export function fetchComponents(request: ShellRequest): Promise<{ components: RecoveryComponent[] }> {
  return getJson(request, "/api/v1/recovery/components");
}

export function fetchSensors(request: ShellRequest): Promise<Sensors> {
  return getJson(request, "/api/v1/recovery/sensors");
}

export function runSelfHeal(request: ShellRequest): Promise<Response> {
  return request("/api/v1/recovery/self-heal", { method: "POST", credentials: "same-origin" });
}

export function restartComponent(request: ShellRequest, target: "shell" | "app", ids?: { appId: string; installationId: string }): Promise<Response> {
  return request("/api/v1/recovery/restart", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ target, ...(ids ?? {}) }),
  });
}

/** Reports a shell activity (navigation, mode, window) to the central log. */
export function reportActivity(request: ShellRequest, action: string): void {
  void request("/api/v1/shell/report", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action: action.slice(0, 200) }),
  }).catch(() => undefined);
}

/** The known audit actions, for the event monitor's filter. */
export const AUDIT_ACTIONS = [
  "sign-in", "sign-in-failed", "sign-out", "reauthenticate", "reauthenticate-failed",
  "os-mode-changed", "app-launched", "app-launch-failed", "app-runtime-started",
  "app-runtime-stopped", "app-installed", "app-uninstalled", "app-imported",
  "capability-invoked", "capability-denied", "app-crashed", "app-unresponsive",
  "service-started", "service-stopped", "service-stale", "service-recovered",
  "self-heal", "maintenance", "system-started", "system-stopped", "activity"
] as const;
