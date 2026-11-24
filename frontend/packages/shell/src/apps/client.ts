import type { ShellRequest } from "../snapshot-client";
import type { BridgeCapabilityOutcome, BridgeCapabilityResource } from "@rumahl/contracts/bridge";
import { APP_ID } from "../routing/paths";
const INSTALLATION = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const LEASE = /^[0-9a-f]{64}$/;
const SETTING_KEY = /^[a-z0-9._-]{1,64}$/;
const SETTING_TYPES = ["text", "number", "boolean", "secret", "select"] as const;
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
  values: Readonly<Record<string, string | number | boolean>>;
  secrets: readonly string[];
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
  /** Runtime kind declared by the app. Container apps can be controlled. */
  runtime?: "web" | "native" | "container";
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
    let runtime: "web" | "native" | "container" | undefined;
    if (app.runtime !== undefined) {
      if (app.runtime !== "web" && app.runtime !== "native" && app.runtime !== "container")
        throw new Error("invalid app runtime kind");
      runtime = app.runtime;
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
    return { id: app.id, installationId: app.installationId, title: app.title, version: app.version, launchable: app.launchable, ...(capabilities ? { capabilities } : {}), ...(runtime ? { runtime } : {}), ...(lifecycle ? { lifecycle } : {}), ...(connectors ? { connectors } : {}) };
  });
}
export async function fetchCatalog(request: ShellRequest, signal: AbortSignal): Promise<readonly InstalledApp[]> {
  return parseCatalog(await payload(await request("/api/v1/shell/apps", {
    cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal
  }), 256 * 1024));
}
export class SafeModeError extends Error {
  constructor() {
    super("safe mode blocks app launches");
    this.name = "SafeModeError";
  }
}
export interface AppLaunch { frameUrl: string; lease: string; renewAfterSeconds: number }
export function parseLaunch(value: unknown, app: InstalledApp, shellOrigin: string, demo?: { frameOrigin: string } | undefined): AppLaunch {
  const object = record(value);
  if (object.launchVersion !== 1 || object.id !== app.id || object.installationId !== app.installationId ||
      typeof object.lease !== "string" || !LEASE.test(object.lease) || typeof object.frameUrl !== "string" || object.frameUrl.length > 2048 ||
      object.renewAfterSeconds !== 30) throw new Error("invalid app launch");
  const url = new URL(object.frameUrl);
  const shell = new URL(shellOrigin);
  const demoFrame = demo !== undefined && url.origin === demo.frameOrigin &&
    url.pathname === `/apps/${encodeURIComponent(app.id)}/`;
  if (demoFrame) {
    if (url.username || url.password || url.search || url.hash)
      throw new Error("invalid demo app origin");
    return { frameUrl: url.href, lease: object.lease, renewAfterSeconds: 30 };
  }
  if (url.protocol !== "https:" || url.hostname === shell.hostname ||
      !url.hostname.startsWith(`${app.installationId}.`) || url.username || url.password || url.search || url.hash ||
      !url.pathname.startsWith(`/launch/${object.lease}/`)) throw new Error("invalid isolated app origin");
  return { frameUrl: url.href, lease: object.lease, renewAfterSeconds: 30 };
}
export async function launchApp(request: ShellRequest, app: InstalledApp, shellOrigin: string, signal: AbortSignal, lease?: string, demo?: { frameOrigin: string } | undefined): Promise<AppLaunch> {
  const response = await request(`/api/v1/shell/apps/${encodeURIComponent(app.id)}/launch`, {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId, ...(lease ? { lease } : {}) })
  });
  if (response.status === 423) throw new SafeModeError();
  return parseLaunch(await payload(response, 4096), app, shellOrigin, demo);
}
export interface SurfaceFrame {
  surfaceId: string;
  appId: string;
  installationId: string;
  frameUrl: string;
  lease: string;
  renewAfterSeconds: number;
}
export function parseSurfaceFrame(value: unknown, app: InstalledApp, surfaceId: string, shellOrigin: string): SurfaceFrame {
  const object = record(value);
  if (object.frameVersion !== 1 || object.surfaceId !== surfaceId || object.appId !== app.id ||
      object.installationId !== app.installationId || typeof object.lease !== "string" || !LEASE.test(object.lease) ||
      typeof object.frameUrl !== "string" || object.frameUrl.length > 2048 || object.renewAfterSeconds !== 30)
    throw new Error("invalid surface frame");
  const url = new URL(object.frameUrl);
  const shell = new URL(shellOrigin);
  if (url.protocol !== "https:" || url.hostname === shell.hostname ||
      !url.hostname.startsWith(`${app.installationId}.`) || url.username || url.password || url.search || url.hash ||
      !url.pathname.startsWith(`/launch/${object.lease}/`)) throw new Error("invalid isolated surface origin");
  return {
    surfaceId, appId: app.id, installationId: app.installationId,
    frameUrl: url.href, lease: object.lease, renewAfterSeconds: 30
  };
}
export async function requestSurfaceFrame(
  request: ShellRequest, app: InstalledApp, surfaceId: string, shellOrigin: string,
  signal: AbortSignal, lease?: string
): Promise<SurfaceFrame> {
  const response = await request(`/api/v1/shell/surfaces/${encodeURIComponent(surfaceId)}/frame`, {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId, ...(lease ? { lease } : {}) })
  });
  if (response.status === 423) throw new SafeModeError();
  return parseSurfaceFrame(await payload(response, 4096), app, surfaceId, shellOrigin);
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
export async function importPackage(request: ShellRequest, files: readonly PackageFile[], device: string, signal: AbortSignal, origin?: "store"): Promise<ImportedApp> {
  return parseImport(await payload(await request("/api/v1/shell/apps/import", {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ importVersion: 1, device, files, ...(origin ? { installOrigin: origin } : {}) })
  }), 4096));
}
export type AppRuntimeAction = "start" | "stop";
export interface AppRuntime {
  id: string;
  installationId: string;
  state: "running" | "stopped";
}
export interface AppRuntimeTarget {
  id: string;
  installationId: string;
}
export function parseAppRuntime(value: unknown, app: AppRuntimeTarget): AppRuntime {
  const object = record(value);
  if (object.runtimeVersion !== 1 || object.id !== app.id ||
      object.installationId !== app.installationId ||
      (object.state !== "running" && object.state !== "stopped"))
    throw new Error("invalid app runtime");
  return { id: app.id, installationId: app.installationId, state: object.state };
}
export async function fetchAppRuntime(request: ShellRequest, app: AppRuntimeTarget, signal: AbortSignal): Promise<AppRuntime> {
  return parseAppRuntime(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/runtime?installationId=${encodeURIComponent(app.installationId)}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 4096), app);
}
export async function controlAppRuntime(request: ShellRequest, app: AppRuntimeTarget, action: AppRuntimeAction, signal: AbortSignal): Promise<AppRuntime> {
  return parseAppRuntime(await payload(await request(`/api/v1/shell/apps/${encodeURIComponent(app.id)}/runtime`, {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId, action })
  }), 4096), app);
}
export async function uninstallApp(request: ShellRequest, app: AppRuntimeTarget, signal: AbortSignal): Promise<void> {
  const response = await request(`/api/v1/shell/apps/${encodeURIComponent(app.id)}`, {
    method: "DELETE", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ installationId: app.installationId })
  });
  if (!response.ok) throw new Error(response.status === 403 ? "denied" : "unavailable");
}
export interface CapabilityBrowserDelivery {
  appId: string;
  installationId: string;
}
export interface CapabilityInvocation {
  capability: string;
  outcome: BridgeCapabilityOutcome;
  result?: unknown;
  /** Set when a web provider must be reached through the shell (its iframe). */
  browser?: CapabilityBrowserDelivery;
}
export function parseCapabilityInvocation(value: unknown, capability: string): CapabilityInvocation {
  const object = record(value);
  if (object.capabilityVersion !== 1 || (object.outcome !== "invoked" && object.outcome !== "denied"))
    throw new Error("invalid capability invocation");
  const result = object.result;
  let browser: CapabilityBrowserDelivery | undefined;
  if (object.browser !== undefined && object.browser !== null) {
    const delivery = record(object.browser);
    if (typeof delivery.appId !== "string" || !APP_ID.test(delivery.appId) ||
        typeof delivery.installationId !== "string" || !INSTALLATION.test(delivery.installationId))
      throw new Error("invalid capability delivery");
    browser = { appId: delivery.appId, installationId: delivery.installationId };
  }
  return {
    capability,
    outcome: object.outcome,
    ...(result === undefined || result === null ? {} : { result }),
    ...(browser ? { browser } : {})
  };
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
  const declared = new Map(manifest.map((setting) => [setting.key, setting]));
  const rawValues = object.values ?? {};
  if (typeof rawValues !== "object" || rawValues === null || Array.isArray(rawValues))
    throw new Error("invalid app setting values");
  const values: Record<string, string | number | boolean> = {};
  for (const [key, value] of Object.entries(rawValues)) {
    const setting = declared.get(key);
    if (!setting || setting.type === "secret") throw new Error("invalid app setting value");
    if (setting.type === "boolean") {
      if (typeof value !== "boolean") throw new Error("invalid app setting value");
    } else if (setting.type === "number") {
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("invalid app setting value");
    } else if (typeof value !== "string") {
      throw new Error("invalid app setting value");
    }
    values[key] = value as string | number | boolean;
  }
  const rawSecrets = object.secrets ?? [];
  if (!Array.isArray(rawSecrets) || rawSecrets.length > 64)
    throw new Error("invalid app setting secrets");
  const secrets: string[] = [];
  for (const key of rawSecrets) {
    const setting = typeof key === "string" ? declared.get(key) : undefined;
    if (!setting || setting.type !== "secret" || secrets.includes(key as string))
      throw new Error("invalid app setting secret");
    secrets.push(key as string);
  }
  return { id: object.id, installationId: object.installationId, manifest, values, secrets, extended: null };
}
export async function updateAppSettings(
  request: ShellRequest,
  app: InstalledApp,
  values: Readonly<Record<string, string | number | boolean | null>>,
  signal: AbortSignal
): Promise<AppSettings> {
  return parseAppSettings(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/settings?installationId=${encodeURIComponent(app.installationId)}`,
    {
      method: "PUT", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify({ values }),
    }
  ), 256 * 1024));
}
export interface AppNetworkOrigin {
  origin: string;
  validity: "session" | "durable";
  expiresAt: number | null;
}
export class NetworkConsentRequiredError extends Error {
  constructor(readonly origin: string) {
    super("network consent required");
    this.name = "NetworkConsentRequiredError";
  }
}
export class NetworkDeniedError extends Error {
  constructor(readonly reason: string) {
    super(reason);
    this.name = "NetworkDeniedError";
  }
}
export interface AppNetworkRequest {
  url: string;
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD";
  headers?: Record<string, string>;
  body?: string;
  secret?: string;
  maxBytes?: number;
}
export interface AppNetworkResponse {
  status: number;
  headers: Record<string, string>;
  body: string;
  truncated: boolean;
}
function networkError(value: unknown): Error {
  const object = typeof value === "object" && value !== null ? value as Record<string, unknown> : {};
  if (object.reason === "approval-required" && typeof object.origin === "string")
    return new NetworkConsentRequiredError(object.origin);
  return new NetworkDeniedError(typeof object.reason === "string" ? object.reason : "network-denied");
}
export async function requestAppNetwork(
  request: ShellRequest,
  app: InstalledApp,
  input: AppNetworkRequest,
  signal: AbortSignal
): Promise<AppNetworkResponse> {
  const response = await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/network/request?installationId=${encodeURIComponent(app.installationId)}`,
    {
      method: "POST", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify(input),
    }
  );
  const value = await response.json().catch(() => null);
  if (!response.ok) throw networkError(value);
  const object = record(value);
  if (object.networkVersion !== 1 || typeof object.status !== "number" ||
      !Number.isInteger(object.status) || object.status < 100 || object.status > 599 ||
      typeof object.body !== "string" || object.body.length > 3 * 1024 * 1024 ||
      typeof object.truncated !== "boolean")
    throw new Error("invalid network response");
  const headers = record(object.headers ?? {});
  const parsedHeaders: Record<string, string> = {};
  for (const [key, entry] of Object.entries(headers)) {
    if (typeof entry !== "string") throw new Error("invalid network response header");
    parsedHeaders[key] = entry;
  }
  return { status: object.status, headers: parsedHeaders, body: object.body, truncated: object.truncated };
}
export async function fetchNetworkOrigins(
  request: ShellRequest,
  app: InstalledApp,
  signal: AbortSignal
): Promise<readonly AppNetworkOrigin[]> {
  const value = record(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/network/consent?installationId=${encodeURIComponent(app.installationId)}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 64 * 1024));
  if (!Array.isArray(value.origins) || value.origins.length > 64) throw new Error("invalid network origins");
  return value.origins.map((entry: unknown) => {
    const origin = record(entry);
    if (typeof origin.origin !== "string" || !origin.origin ||
        (origin.validity !== "session" && origin.validity !== "durable") ||
        !(origin.expiresAt === null || (typeof origin.expiresAt === "number" && Number.isInteger(origin.expiresAt))))
      throw new Error("invalid network origin");
    return { origin: origin.origin, validity: origin.validity, expiresAt: origin.expiresAt } as AppNetworkOrigin;
  });
}
export async function grantNetworkOrigin(
  request: ShellRequest,
  app: InstalledApp,
  origin: string,
  validity: "session" | "durable",
  signal: AbortSignal
): Promise<AppNetworkOrigin> {
  const value = record(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/network/consent?installationId=${encodeURIComponent(app.installationId)}`,
    {
      method: "PUT", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify({ origin, validity }),
    }
  ), 64 * 1024));
  if (typeof value.origin !== "string" || (value.validity !== "session" && value.validity !== "durable") ||
      !(value.expiresAt === null || typeof value.expiresAt === "number"))
    throw new Error("invalid network origin");
  return { origin: value.origin, validity: value.validity, expiresAt: value.expiresAt };
}
export async function revokeNetworkOrigin(
  request: ShellRequest,
  app: InstalledApp,
  origin: string,
  signal: AbortSignal
): Promise<void> {
  const response = await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/network/consent?installationId=${encodeURIComponent(app.installationId)}&origin=${encodeURIComponent(origin)}`,
    { method: "DELETE", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json" } }
  );
  if (!response.ok) throw new Error("network consent could not be revoked");
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
export interface AppImageEntry { name: string; value: string }
export interface AppImageEnvironment {
  runtime: "web" | "native" | "container";
  image: string | null;
  entries: AppImageEntry[];
}
export function parseAppImageEnvironment(value: unknown, app: AppRuntimeTarget): AppImageEnvironment {
  const object = record(value);
  if (typeof object.id !== "string" || typeof object.installationId !== "string" ||
      object.id !== app.id || object.installationId !== app.installationId ||
      (object.runtime !== "web" && object.runtime !== "native" && object.runtime !== "container") ||
      (object.image !== null && typeof object.image !== "string") ||
      (object.image !== null && object.image.length > 512) || !Array.isArray(object.entries))
    throw new Error("invalid app image environment");
  const entries: AppImageEntry[] = [];
  for (const entry of object.entries) {
    const item = record(entry);
    if (entries.length >= 64 || typeof item.name !== "string" || typeof item.value !== "string" ||
        !/^[A-Za-z0-9_.-]{1,128}$/.test(item.name) || item.value.length > 512)
      throw new Error("invalid app image environment entry");
    entries.push({ name: item.name, value: item.value });
  }
  return { runtime: object.runtime, image: object.image, entries };
}
export async function fetchAppImageEnvironment(request: ShellRequest, app: AppRuntimeTarget, device: string, signal: AbortSignal): Promise<AppImageEnvironment> {
  const query = new URLSearchParams({ installationId: app.installationId, device });
  return parseAppImageEnvironment(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/image?${query.toString()}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 16 * 1024), app);
}
/** One manifest permission request, for the consent view. */
export interface AppPermissionRequestInfo {
  permission: string;
  scope: string;
  required: boolean;
  reason?: string;
}
/** One active grant issued by the calling user. */
export interface AppGrantInfo {
  id: string;
  permission: string;
  scope: string;
  resources: string[];
  validity: "session" | "durable";
  expiresAt: number | null;
}
export interface AppGrantState {
  requests: AppPermissionRequestInfo[];
  grants: AppGrantInfo[];
}
export function parseAppGrantState(value: unknown, app: AppRuntimeTarget): AppGrantState {
  const object = record(value);
  if (object.id !== app.id || object.installationId !== app.installationId ||
      !Array.isArray(object.requests) || !Array.isArray(object.grants))
    throw new Error("invalid app grant state");
  const requests: AppPermissionRequestInfo[] = [];
  for (const entry of object.requests) {
    const item = record(entry);
    if (requests.length >= 64 || typeof item.permission !== "string" || item.permission.length > 255 ||
        typeof item.scope !== "string" || item.scope.length > 32 || typeof item.required !== "boolean" ||
        (item.reason !== undefined && typeof item.reason !== "string") || (item.reason as string).length > 512)
      throw new Error("invalid app permission request");
    requests.push({
      permission: item.permission, scope: item.scope, required: item.required,
      ...(item.reason === undefined ? {} : { reason: item.reason })
    });
  }
  const grants: AppGrantInfo[] = [];
  for (const entry of object.grants) {
    const item = record(entry);
    if (grants.length >= 64 || typeof item.id !== "string" || !/^[0-9a-f-]{36}$/.test(item.id) ||
        typeof item.permission !== "string" || item.permission.length > 255 ||
        typeof item.scope !== "string" || item.scope.length > 32 ||
        (item.validity !== "session" && item.validity !== "durable") ||
        (item.expiresAt !== null && typeof item.expiresAt !== "number") ||
        !Array.isArray(item.resources) || item.resources.length > 64 ||
        item.resources.some((resource: unknown) => typeof resource !== "string" || resource.length > 320))
      throw new Error("invalid app grant");
    grants.push({
      id: item.id, permission: item.permission, scope: item.scope, resources: item.resources,
      validity: item.validity, expiresAt: item.expiresAt
    });
  }
  return { requests, grants };
}
export async function fetchAppGrantState(request: ShellRequest, app: AppRuntimeTarget, signal: AbortSignal): Promise<AppGrantState> {
  return parseAppGrantState(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/grants?installationId=${encodeURIComponent(app.installationId)}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 4096), app);
}
export interface GrantIssueInput {
  permission: "rumahl.files.read" | "rumahl.files.write";
  resources: string[];
  validity: "session" | "durable";
  /** Seconds from now; only valid with `durable`. */
  expiresInSeconds?: number;
  /** Required for a durable `rumahl.files.write` consent. */
  password?: string;
}
export async function issueAppGrant(request: ShellRequest, app: AppRuntimeTarget, issue: GrantIssueInput, signal: AbortSignal): Promise<AppGrantInfo> {
  const body: Record<string, unknown> = {
    permission: issue.permission,
    resources: issue.resources,
    validity: issue.validity,
    ...(issue.expiresInSeconds === undefined ? {} : { expiresIn: issue.expiresInSeconds }),
    ...(issue.password === undefined ? {} : { password: issue.password })
  };
  return parseAppGrant(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/grants?installationId=${encodeURIComponent(app.installationId)}`,
    { method: "PUT", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify(body) }
  ), 4096), app);
}
function parseAppGrant(value: unknown, app: AppRuntimeTarget): AppGrantInfo {
  const object = record(value);
  if (object.id !== app.id || object.installationId !== app.installationId ||
      typeof object.grantId !== "string" || !/^[0-9a-f-]{36}$/.test(object.grantId) ||
      typeof object.permission !== "string" || (object.validity !== "session" && object.validity !== "durable") ||
      !Array.isArray(object.resources) || object.resources.length > 64 ||
      object.resources.some((resource: unknown) => typeof resource !== "string" || resource.length > 320))
    throw new Error("invalid app grant");
  return {
    id: object.grantId, permission: object.permission, scope: "explicit",
    resources: object.resources, validity: object.validity, expiresAt: null
  };
}
export async function revokeAppGrant(request: ShellRequest, app: AppRuntimeTarget, grantId: string, signal: AbortSignal): Promise<void> {
  if (!/^[0-9a-f-]{36}$/.test(grantId)) throw new Error("invalid grant id");
  const response = await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/grants/${encodeURIComponent(grantId)}?installationId=${encodeURIComponent(app.installationId)}`,
    { method: "DELETE", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json" } }
  );
  if (!response.ok && response.status !== 204) throw new Error(response.status === 403 ? "denied" : "unavailable");
}
export interface AppStorage {
  usedBytes: number;
  quotaBytes: number;
}
export function parseAppStorage(value: unknown, app: AppRuntimeTarget): AppStorage {
  const object = record(value);
  if (object.id !== app.id || object.installationId !== app.installationId ||
      typeof object.usedBytes !== "number" || !Number.isSafeInteger(object.usedBytes) || object.usedBytes < 0 ||
      typeof object.quotaBytes !== "number" || !Number.isSafeInteger(object.quotaBytes) || object.quotaBytes <= 0 ||
      object.usedBytes > object.quotaBytes * 1024)
    throw new Error("invalid app storage");
  return { usedBytes: object.usedBytes, quotaBytes: object.quotaBytes };
}
export async function fetchAppStorage(request: ShellRequest, app: AppRuntimeTarget, device: string, signal: AbortSignal): Promise<AppStorage> {
  const query = new URLSearchParams({ installationId: app.installationId, device });
  return parseAppStorage(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/storage?${query.toString()}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 4096), app);
}
/** Reads one personal file through the app's grants. */
export async function readAppFileContent(request: ShellRequest, app: AppRuntimeTarget, key: string, signal: AbortSignal): Promise<{ name: string; bytes: Uint8Array }> {
  const response = await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/files/content?installationId=${encodeURIComponent(app.installationId)}&key=${encodeURIComponent(key)}`,
    { cache: "no-store", credentials: "same-origin", signal, headers: { Accept: "application/octet-stream" } }
  );
  if (!response.ok) throw new Error(response.status === 403 ? "denied" : "unavailable");
  const buffer = new Uint8Array(await response.arrayBuffer());
  if (buffer.length > 16 * 1024 * 1024) throw new Error("unavailable");
  const name = dispositionFilename(response.headers.get("content-disposition")) ?? "file";
  return { name, bytes: buffer };
}
/** Writes one personal file through the app's grants. */
export async function writeAppFileContent(
  request: ShellRequest, app: AppRuntimeTarget, key: string, bytes: Uint8Array, signal: AbortSignal
): Promise<{ name: string; size: number }> {
  const response = await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/files/content?installationId=${encodeURIComponent(app.installationId)}&key=${encodeURIComponent(key)}`,
    { method: "PUT", cache: "no-store", credentials: "same-origin", signal,
      headers: { Accept: "application/json", "Content-Type": "application/octet-stream" },
      body: new Blob([bytes as BlobPart]) }
  );
  if (!response.ok) throw new Error(response.status === 403 ? "denied" : "unavailable");
  const object = record(await payload(response, 4096));
  if (object.outcome !== "allowed" || typeof object.name !== "string" || typeof object.size !== "number")
    throw new Error("invalid app file write");
  return { name: object.name, size: object.size };
}
function dispositionFilename(disposition: string | null): string | null {
  if (!disposition) return null;
  const match = /filename="((?:[^"\\]|\\.)*)"/.exec(disposition);
  return match?.[1] ?? null;
}
export async function fetchAppData(request: ShellRequest, app: InstalledApp, path: string, device: string, signal: AbortSignal): Promise<AppData> {
  const query = new URLSearchParams({ installationId: app.installationId, device, path });
  return parseAppData(await payload(await request(
    `/api/v1/shell/apps/${encodeURIComponent(app.id)}/data?${query.toString()}`,
    { cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal }
  ), 512 * 1024));
}
