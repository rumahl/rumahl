import type { ShellRequest } from "../snapshot-client";
import { APP_ID } from "../routing/paths";
import { parseImport, type ImportedApp } from "./client";

export type StoreSourceKind = "website" | "github";
export interface StoreSource {
  id: string;
  kind: StoreSourceKind;
  location: string;
  title: string;
  status: "ready" | "unavailable";
  error?: string | null;
}
export interface StoreApp {
  sourceId: string;
  id: string;
  title: string;
  version: string;
  description?: string | null;
  publisher?: string | null;
  icon?: string | null;
}
export interface StoreListing {
  sources: readonly StoreSource[];
  apps: readonly StoreApp[];
}
const SOURCE_ID = /^[0-9a-f]{32}$/;
const MAX_SOURCES = 64;
const MAX_APPS = 16384;
function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || !value || Array.isArray(value)) throw new Error("invalid store response");
  return value as Record<string, unknown>;
}
function optionalString(value: unknown, maximum: number): string | null | undefined {
  if (value === undefined || value === null) return value;
  if (typeof value !== "string" || value.length > maximum) throw new Error("invalid store text");
  return value;
}
export function parseStoreSource(value: unknown): StoreSource {
  const source = record(value);
  if (typeof source.id !== "string" || !SOURCE_ID.test(source.id) ||
      (source.kind !== "website" && source.kind !== "github") ||
      typeof source.location !== "string" || source.location.length === 0 || source.location.length > 2048 ||
      typeof source.title !== "string" || source.title.length === 0 || source.title.length > 120 ||
      (source.status !== "ready" && source.status !== "unavailable")) throw new Error("invalid store source");
  const error = optionalString(source.error, 500);
  return {
    id: source.id,
    kind: source.kind,
    location: source.location,
    title: source.title,
    status: source.status,
    ...(typeof error === "string" ? { error } : {}),
  };
}
export function parseStoreApp(value: unknown): StoreApp {
  const app = record(value);
  if (typeof app.sourceId !== "string" || !SOURCE_ID.test(app.sourceId) ||
      typeof app.id !== "string" || !APP_ID.test(app.id) ||
      typeof app.title !== "string" || app.title.length === 0 || app.title.length > 120 ||
      typeof app.version !== "string" || app.version.length === 0 || app.version.length > 128)
    throw new Error("invalid store app");
  optionalString(app.description, 2000);
  optionalString(app.publisher, 128);
  optionalString(app.icon, 2048);
  return {
    sourceId: app.sourceId,
    id: app.id,
    title: app.title,
    version: app.version,
    ...(typeof app.description === "string" ? { description: app.description } : {}),
    ...(typeof app.publisher === "string" ? { publisher: app.publisher } : {}),
    ...(typeof app.icon === "string" ? { icon: app.icon } : {}),
  };
}
export function parseStoreListing(value: unknown): StoreListing {
  const listing = record(value);
  if (listing.storeVersion !== 1 || !Array.isArray(listing.sources) || listing.sources.length > MAX_SOURCES ||
      !Array.isArray(listing.apps) || listing.apps.length > MAX_APPS) throw new Error("invalid store listing");
  const sources = listing.sources.map(parseStoreSource);
  const sourceIds = new Set(sources.map((source) => source.id));
  if (sourceIds.size !== sources.length) throw new Error("invalid store source list");
  const appIds = new Set<string>();
  const apps = listing.apps.map(parseStoreApp);
  if (apps.some((app) => {
    const key = `${app.sourceId}:${app.id}`;
    if (!sourceIds.has(app.sourceId) || appIds.has(key)) return true;
    appIds.add(key);
    return false;
  })) throw new Error("invalid store app source");
  return { sources, apps };
}
async function payload(response: Response, maximum: number): Promise<unknown> {
  if (!response.ok) throw new Error("store request unavailable");
  const text = await response.text();
  if (text.length > maximum) throw new Error("store response too large");
  return JSON.parse(text);
}
export async function fetchStore(request: ShellRequest, signal: AbortSignal): Promise<StoreListing> {
  return parseStoreListing(await payload(await request("/api/v1/shell/store", {
    cache: "no-store", credentials: "same-origin", headers: { Accept: "application/json" }, signal
  }), 16 * 1024 * 1024));
}
export async function addStoreSource(request: ShellRequest, kind: StoreSourceKind, location: string, signal: AbortSignal): Promise<StoreSource> {
  const response = await payload(await request("/api/v1/shell/store/sources", {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ kind, location })
  }), 64 * 1024);
  const object = record(response);
  if (object.storeVersion !== 1) throw new Error("invalid store source response");
  return parseStoreSource(object.source);
}
export async function removeStoreSource(request: ShellRequest, id: string, signal: AbortSignal): Promise<void> {
  const response = await request(`/api/v1/shell/store/sources/${encodeURIComponent(id)}`, {
    method: "DELETE", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json" }
  });
  if (!response.ok) throw new Error("store source could not be removed");
}
export async function installStoreApp(request: ShellRequest, sourceId: string, appId: string, device: string, signal: AbortSignal): Promise<ImportedApp> {
  return parseImport(await payload(await request("/api/v1/shell/store/install", {
    method: "POST", cache: "no-store", credentials: "same-origin", signal,
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ sourceId, appId, device })
  }), 4096));
}
