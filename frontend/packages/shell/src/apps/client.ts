import type { ShellRequest } from "../snapshot-client";
import type { BridgeCapabilityOutcome, BridgeCapabilityResource } from "@rumahl/contracts/bridge";
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
export interface AppDataEntry {
  name: string;
  directory: boolean;
  size: number;
}
export type AppData =
  | { kind: "directory"; path: string; entries: readonly AppDataEntry[] }
  | { kind: "file"; path: string; size: number; contentType: string; text: string | null };
export interface InstalledApp {
  id: string;
  installationId: string;
  title: string;
  version: string;
  launchable: boolean;
  /** Permission ids the app's manifest declares (used to gate the OS bridge). */
  capabilities?: readonly string[];
  /** Runtime lifecycle declared by the app: `always-on` (service) or `on-demand`. */
  lifecycle?: "always-on" | "on-demand";
  /** External services the app ships a connector bundle for. */
  connectors?: readonly string[];
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
    let capabilities: readonly string[] | undefined;
    if (app.capabilities !== undefined) {
      if (!Array.isArray(app.capabilities) || app.capabilities.length > 64 ||
          app.capabilities.some((entry) => typeof entry !== "string" || entry.length === 0 || entry.length > 255)) throw new Error("invalid app capabilities");
      capabilities = app.capabilities as string[];
    }
    let lifecycle: "always-on" | "on-demand" | undefined;
    if (app.lifecycle !== undefined) {
      if (app.lifecycle !== "always-on" && app.lifecycle !== "on-demand") throw new Error("invalid app lifecycle");
      lifecycle = app.lifecycle;
    }
    let connectors: readonly string[] | undefined;
    if (app.connectors !== undefined) {
      if (!Array.isArray(app.connectors) || app.connectors.length > 64 ||
          app.connectors.some((entry) => typeof entry !== "string" || entry.length === 0 || entry.length > 64)) throw new Error("invalid app connectors");
      connectors = app.connectors as string[];
    }
    ids.add(app.id);
    return { id: app.id, installationId: app.installationId, title: app.title, version: app.version, launchable: app.launchable, ...(capabilities ? { capabilities } : {}), ...(lifecycle ? { lifecycle } : {}), ...(connectors ? { connectors } : {}) };
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
/** One file of a signed directory package, base64 encoded, path relative to the package root. */
export interface PackageFile { path: string; content: string }
export interface ImportedApp { id: string; installationId: string; title: string; version: string }
export function parseImport(value: unknown): ImportedApp {
  const object = record(value);
  if (object.importVersion !== 1 || typeof object.id !== "string" || !APP_ID.test(object.id) ||
      typeof object.installationId !== "string" || !INSTALLATION.test(object.installationId) ||
      typeof object.title !== "string" || !object.title.trim() || object.title.length > 256 ||
      typeof object.version !== "string" || object.version.length > 128) throw new Error("invalid app import");
  return { id: object.id, installationId: object.installationId, title: object.title, version: object.version };
}
export async function importPackage(request: ShellRequest, files: readonly PackageFile[], device: string, signal: AbortSignal): Promise<ImportedApp> {
  return parseImport(await payload(await request("/api/v1/shell/apps/import", {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ importVersion: 1, device, files })
  }), 4096));
}
export type AppRuntimeAction = "start" | "stop";
export async function controlAppRuntime(request: ShellRequest, app: InstalledApp, action: AppRuntimeAction, signal: AbortSignal): Promise<void> {
  await payload(await request(`/api/v1/shell/apps/${encodeURIComponent(app.id)}/runtime`, {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId, action })
  }), 4096);
}
export interface CapabilityInvocation {
  capability: string;
  outcome: BridgeCapabilityOutcome;
}
export function parseCapabilityInvocation(value: unknown, capability: string): CapabilityInvocation {
  const object = record(value);
  if (object.capabilityVersion !== 1 || (object.outcome !== "invoked" && object.outcome !== "denied"))
    throw new Error("invalid capability invocation");
  return { capability, outcome: object.outcome };
}
export async function invokeCapability(request: ShellRequest, capability: string, resource: BridgeCapabilityResource | undefined, signal: AbortSignal): Promise<CapabilityInvocation> {
  return parseCapabilityInvocation(await payload(await request("/api/v1/shell/capabilities/invoke", {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ capability, ...(resource ? { resource } : {}) })
  }), 4096), capability);
}
/** Reads selected package files into base64 payloads, stripping the chosen folder prefix. */
export async function readPackageFiles(selected: readonly File[]): Promise<PackageFile[]> {
  const files: PackageFile[] = [];
  for (const file of selected) {
    const relative = (file.webkitRelativePath || file.name).split("/").slice(1).join("/") || file.name;
    if (!relative || relative.split("/").some((segment) => segment === ".." || segment === "")) continue;
    const bytes = new Uint8Array(await file.arrayBuffer());
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    files.push({ path: relative, content: btoa(binary) });
  }
  return files;
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
export function parseAppData(value: unknown): AppData {
  const object = record(value);
  if (object.dataVersion !== 1 || typeof object.path !== "string" || object.path.length > 2048 ||
      object.path.includes("\0") ||
      object.path.split("/").some((segment) => segment === "." || segment === ".." || segment.includes("\\")))
    throw new Error("invalid app data");
  if (object.kind === "directory") {
    if (!Array.isArray(object.entries) || object.entries.length > 1024) throw new Error("invalid app data");
    const entries = object.entries.map((value: unknown) => {
      const entry = record(value);
      if (typeof entry.name !== "string" || !entry.name || entry.name.length > 255 ||
          entry.name === "." || entry.name === ".." || entry.name.includes("/") || entry.name.includes("\0") ||
          typeof entry.directory !== "boolean" ||
          typeof entry.size !== "number" || !Number.isInteger(entry.size) || entry.size < 0 || entry.size > 64 * 1024 * 1024)
        throw new Error("invalid app data entry");
      return { name: entry.name, directory: entry.directory, size: entry.size };
    });
    return { kind: "directory", path: object.path, entries };
  }
  if (object.kind === "file") {
    if (typeof object.contentType !== "string" || object.contentType.length > 128 ||
        typeof object.size !== "number" || !Number.isInteger(object.size) || object.size < 0 || object.size > 2 * 1024 * 1024 ||
        !(object.text === null || typeof object.text === "string"))
      throw new Error("invalid app data file");
    return { kind: "file", path: object.path, size: object.size, contentType: object.contentType, text: object.text };
  }
  throw new Error("invalid app data kind");
}
export async function fetchAppData(request: ShellRequest, app: InstalledApp, path: string, device: string, signal: AbortSignal): Promise<AppData> {
  const query = new URLSearchParams({ installationId: app.installationId, device, path });
  return parseAppData(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/data?${query.toString()}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 512 * 1024));
}
