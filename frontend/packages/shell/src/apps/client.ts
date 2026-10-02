import type { ShellRequest } from "../snapshot-client";
import { APP_ID } from "../routing/paths";
const INSTALLATION = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const LEASE = /^[0-9a-f]{64}$/;
export interface InstalledApp {
  id: string;
  installationId: string;
  title: string;
  version: string;
  launchable: boolean;
}
function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || !value || Array.isArray(value)) throw new Error("invalid app response");
  return value as Record<string, unknown>;
}
async function payload(response: Response, maximum: number): Promise<unknown> {
  if (!response.ok) throw new Error("app request unavailable");
  const text = await response.text();
  if (text.length > maximum) throw new Error("app response too large");
  return JSON.parse(text);
}
export function parseCatalog(value: unknown): readonly InstalledApp[] {
  const object = record(value);
  if (object.catalogVersion !== 1 || !Array.isArray(object.apps) || object.apps.length > 256) throw new Error("invalid app catalog");
  const ids = new Set<string>();
  return object.apps.map((value: unknown) => {
    const app = record(value);
    if (typeof app.id !== "string" || !APP_ID.test(app.id) || ids.has(app.id) ||
        typeof app.installationId !== "string" || !INSTALLATION.test(app.installationId) ||
        typeof app.title !== "string" || !app.title.trim() || app.title.length > 256 ||
        typeof app.version !== "string" || app.version.length > 128 || typeof app.launchable !== "boolean") throw new Error("invalid app entry");
    ids.add(app.id);
    return { id: app.id, installationId: app.installationId, title: app.title, version: app.version, launchable: app.launchable };
  });
}
export async function fetchCatalog(request: ShellRequest, signal: AbortSignal): Promise<readonly InstalledApp[]> {
  return parseCatalog(await payload(await request("/api/v1/shell/apps", {
    cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal
  }), 256 * 1024));
}
export interface AppLaunch { frameUrl: string; lease: string; renewAfterSeconds: number }
export function parseLaunch(value: unknown, app: InstalledApp, shellOrigin: string): AppLaunch {
  const object = record(value);
  if (object.launchVersion !== 1 || object.id !== app.id || object.installationId !== app.installationId ||
      typeof object.lease !== "string" || !LEASE.test(object.lease) || typeof object.frameUrl !== "string" || object.frameUrl.length > 2048 ||
      object.renewAfterSeconds !== 30) throw new Error("invalid app launch");
  const url = new URL(object.frameUrl);
  const shell = new URL(shellOrigin);
  if (url.protocol !== "https:" || url.hostname === shell.hostname ||
      !url.hostname.startsWith(`${app.installationId}.`) || url.username || url.password || url.search || url.hash ||
      !url.pathname.startsWith(`/launch/${object.lease}/`)) throw new Error("invalid isolated app origin");
  return { frameUrl: url.href, lease: object.lease, renewAfterSeconds: 30 };
}
export async function launchApp(request: ShellRequest, app: InstalledApp, shellOrigin: string, signal: AbortSignal, lease?: string): Promise<AppLaunch> {
  return parseLaunch(await payload(await request(`/api/v1/shell/apps/${encodeURIComponent(app.id)}/launch`, {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId, ...(lease ? { lease } : {}) })
  }), 4096), app, shellOrigin);
}
