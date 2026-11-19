import type { ShellRequest } from "../snapshot-client";
import type { BridgeWeatherWarning } from "@rumahl/contracts/bridge";

export type WeatherWarningLevel = "info" | "warning" | "critical" | "emergency";

export interface WeatherWarningSource {
  id: string;
  name: string;
  registeredAt: number;
}

export interface WeatherWarning {
  id: string;
  source: WeatherWarningSource;
  level: WeatherWarningLevel;
  headline: string;
  description: string;
  region: string;
  event: string;
  severity: string;
  sender: string;
  instruction: string;
  area: string;
  effective: number | null;
  expires: number;
  test: boolean;
  publishedAt: number;
  updatedAt: number;
}

export interface WeatherWarningsState {
  revision: string;
  observedAt: number;
  warnings: readonly WeatherWarning[];
}

const LEVELS: readonly WeatherWarningLevel[] = ["info", "warning", "critical", "emergency"];
const BROWSER_PROFILE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("invalid warning response");
  return value as Record<string, unknown>;
}

async function payload(response: Response, maximum = 128 * 1024): Promise<unknown> {
  if (!response.ok) throw new Error(`warning request failed: ${response.status}`);
  const text = await response.text();
  if (text.length > maximum) throw new Error("warning response too large");
  return JSON.parse(text);
}

function string(value: unknown, maximum: number, allowEmpty = true): string {
  if (typeof value !== "string" || value.length > maximum || (!allowEmpty && value.trim().length === 0)) {
    throw new Error("invalid warning text");
  }
  return value;
}

function timestamp(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) throw new Error("invalid warning timestamp");
  return value;
}

function source(value: unknown): WeatherWarningSource {
  const entry = object(value);
  return {
    id: string(entry.id, 255, false),
    name: string(entry.name, 200, false),
    registeredAt: timestamp(entry.registeredAt)
  };
}

function warning(value: unknown): WeatherWarning {
  const entry = object(value);
  const level = string(entry.level, 16, false);
  if (!(LEVELS as readonly string[]).includes(level)) throw new Error("invalid warning level");
  const effective = entry.effective === null ? null : timestamp(entry.effective);
  if (typeof entry.test !== "boolean") throw new Error("invalid warning test flag");
  return {
    id: string(entry.id, 64, false),
    source: source(entry.source),
    level: level as WeatherWarningLevel,
    headline: string(entry.headline, 160, false),
    description: string(entry.description, 2048),
    region: string(entry.region, 120),
    event: string(entry.event, 120),
    severity: string(entry.severity, 64),
    sender: string(entry.sender, 120),
    instruction: string(entry.instruction, 1024),
    area: string(entry.area, 512),
    effective,
    expires: timestamp(entry.expires),
    test: entry.test,
    publishedAt: timestamp(entry.publishedAt),
    updatedAt: timestamp(entry.updatedAt)
  };
}

export function parseWarningsState(value: unknown): WeatherWarningsState {
  const entry = object(value);
  if (entry.warningsVersion !== 1 || typeof entry.revision !== "string" || !/^[A-Za-z0-9._-]{1,128}$/.test(entry.revision)) {
    throw new Error("invalid warning state");
  }
  if (!Array.isArray(entry.warnings) || entry.warnings.length > 64) throw new Error("invalid warning list");
  const ids = new Set<string>();
  const warnings = entry.warnings.map((item) => {
    const parsed = warning(item);
    if (ids.has(parsed.id)) throw new Error("duplicate warning");
    ids.add(parsed.id);
    return parsed;
  });
  return {
    revision: entry.revision,
    observedAt: timestamp(entry.observedAt),
    warnings
  };
}

export async function fetchWeatherWarnings(request: ShellRequest, signal: AbortSignal): Promise<WeatherWarningsState> {
  return parseWarningsState(await payload(await request("/api/v1/shell/warnings", {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal
  })));
}

export async function registerWarningSource(request: ShellRequest, appId: string, signal: AbortSignal): Promise<WeatherWarningSource> {
  const entry = object(await payload(await request("/api/v1/shell/warnings/register", {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ appId }),
    signal
  }), 4096));
  if (entry.registrationVersion !== 1) throw new Error("invalid warning registration");
  return source(entry.source);
}

export async function publishWeatherWarnings(
  request: ShellRequest,
  appId: string,
  warnings: readonly BridgeWeatherWarning[],
  signal: AbortSignal
): Promise<number> {
  const entry = object(await payload(await request("/api/v1/shell/warnings/publish", {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ appId, warnings }),
    signal
  })));
  if (entry.publishVersion !== 1 || typeof entry.accepted !== "number" || !Number.isInteger(entry.accepted) || entry.accepted < 1 || entry.accepted > 8) {
    throw new Error("invalid warning publish result");
  }
  return entry.accepted;
}

export async function clearWeatherWarnings(
  request: ShellRequest,
  appId: string,
  ids: readonly string[] | undefined,
  signal: AbortSignal
): Promise<number> {
  const entry = object(await payload(await request("/api/v1/shell/warnings/clear", {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ appId, ...(ids ? { ids } : {}) }),
    signal
  })));
  if (entry.clearVersion !== 1 || typeof entry.cleared !== "number" || !Number.isInteger(entry.cleared) || entry.cleared < 0 || entry.cleared > 64) {
    throw new Error("invalid warning clear result");
  }
  return entry.cleared;
}

/**
 * Developer-only diagnostic. The server checks the effective OS mode for this
 * device and publishes a clearly marked test warning through the native source.
 */
export async function publishTestWarning(
  request: ShellRequest,
  device: string,
  level: WeatherWarningLevel,
  signal: AbortSignal,
  extreme = false
): Promise<number> {
  if (!BROWSER_PROFILE.test(device)) throw new Error("invalid browser profile");
  const entry = object(await payload(await request(`/api/v1/shell/warnings/test?device=${encodeURIComponent(device)}`, {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ level, ...(extreme ? { extreme: true } : {}) }),
    signal
  })));
  if (entry.publishVersion !== 1 || entry.accepted !== 1) throw new Error("invalid test warning result");
  return 1;
}
