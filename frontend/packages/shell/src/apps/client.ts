import type { ShellRequest } from "../snapshot-client";
import { APP_ID } from "../routing/paths";
const INSTALLATION = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const LEASE = /^[0-9a-f]{64}$/;
const SETTING_KEY = /^[a-z0-9._-]{1,64}$/;
const SETTING_TYPES = ["text", "number", "boolean", "select"] as const;
export type AppSettingType = (typeof SETTING_TYPES)[number];
export interface AppSettingOption {
  value: string;
  label: string;
}
export interface AppSetting {
  key: string;
  title: string;
  description?: string;
  type: AppSettingType;
  required: boolean;
  options: readonly AppSettingOption[];
}
export interface AppSettings {
  id: string;
  installationId: string;
  manifest: readonly AppSetting[];
  extended: null;
}
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
export function parseAppSettings(value: unknown): AppSettings {
  const object = record(value);
  if (object.settingsVersion !== 1 || typeof object.id !== "string" || !APP_ID.test(object.id) ||
      typeof object.installationId !== "string" || !INSTALLATION.test(object.installationId) ||
      !Array.isArray(object.manifest) || object.manifest.length > 64 || object.extended !== null)
    throw new Error("invalid app settings");
  const keys = new Set<string>();
  const manifest = object.manifest.map((value: unknown) => {
    const setting = record(value);
    const type = setting.type;
    const description = setting.description;
    if (typeof setting.key !== "string" || !SETTING_KEY.test(setting.key) || keys.has(setting.key) ||
        typeof setting.title !== "string" || !setting.title.trim() || setting.title.length > 120 ||
        !(description === null || description === undefined || (typeof description === "string" && description.length <= 500)) ||
        typeof type !== "string" || !(SETTING_TYPES as readonly string[]).includes(type) ||
        typeof setting.required !== "boolean" || !Array.isArray(setting.options) || setting.options.length > 64)
      throw new Error("invalid app setting");
    keys.add(setting.key);
    const options = setting.options.map((value: unknown) => {
      const option = record(value);
      if (typeof option.value !== "string" || !option.value || option.value.length > 128 ||
          typeof option.label !== "string" || !option.label.trim() || option.label.length > 120)
        throw new Error("invalid app setting option");
      return { value: option.value, label: option.label };
    });
    if (type === "select" && !options.length) throw new Error("select setting without options");
    if (type !== "select" && options.length) throw new Error("options on a non-select setting");
    return {
      key: setting.key,
      title: setting.title,
      ...(typeof description === "string" ? { description } : {}),
      type: type as AppSettingType,
      required: setting.required,
      options,
    };
  });
  return { id: object.id, installationId: object.installationId, manifest, extended: null };
}
export async function fetchAppSettings(request: ShellRequest, app: InstalledApp, signal: AbortSignal): Promise<AppSettings> {
  return parseAppSettings(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/settings?installationId=${encodeURIComponent(app.installationId)}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 256 * 1024));
}
