import type { ShellRequest } from "../snapshot-client";

export interface HostEntry {
  name: string;
  directory: boolean;
  size: number;
  modified: number;
}
export type HostArea = "apps" | "system" | "home";

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || !value || Array.isArray(value)) throw new Error("invalid host response");
  return value as Record<string, unknown>;
}
export function parseHostList(value: unknown): readonly HostEntry[] {
  const object = record(value);
  if (object.fsVersion !== 1 ||
      (object.area !== "apps" && object.area !== "system" && object.area !== "home") ||
      typeof object.path !== "string" || object.path.length > 4096 ||
      !Array.isArray(object.entries) || object.entries.length > 2000)
    throw new Error("invalid host listing");
  return object.entries.map((value: unknown) => {
    const entry = record(value);
    if (typeof entry.name !== "string" || !entry.name || entry.name.length > 255 ||
        entry.name === "." || entry.name === ".." || entry.name.includes("/") || entry.name.includes("\0") ||
        typeof entry.directory !== "boolean" ||
        typeof entry.size !== "number" || !Number.isInteger(entry.size) || entry.size < 0 ||
        typeof entry.modified !== "number" || !Number.isInteger(entry.modified) || entry.modified < 0)
      throw new Error("invalid host entry");
    return { name: entry.name, directory: entry.directory, size: entry.size, modified: entry.modified };
  });
}
export async function fetchHostList(
  request: ShellRequest,
  area: HostArea,
  path: string,
  device: string,
  signal: AbortSignal,
): Promise<readonly HostEntry[]> {
  const query = new URLSearchParams({ area, path, device });
  const response = await request(`/api/v1/shell/fs?${query.toString()}`, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) throw new Error("host listing unavailable");
  const text = await response.text();
  if (text.length > 512 * 1024) throw new Error("host response too large");
  return parseHostList(JSON.parse(text));
}
export function hostContentUrl(area: HostArea, path: string, device: string): string {
  const query = new URLSearchParams({ area, path, device });
  return `/api/v1/shell/fs/content?${query.toString()}`;
}

function fsFailure(status: number): Error {
  return new Error(status === 404 ? "missing" : status === 413 ? "limit" : status === 409 ? "conflict" : "unavailable");
}

/** Replaces or creates one file in the user's home area. */
export async function writeHostFile(
  request: ShellRequest,
  path: string,
  bytes: BlobPart,
  device: string,
  signal: AbortSignal,
): Promise<void> {
  const query = new URLSearchParams({ area: "home", path, device });
  const response = await request(`/api/v1/shell/fs/content?${query.toString()}`, {
    method: "PUT",
    cache: "no-store",
    credentials: "same-origin",
    headers: { "Content-Type": "application/octet-stream" },
    body: new Blob([bytes]),
    signal,
  });
  if (!response.ok) throw fsFailure(response.status);
}

/** Creates one file or directory in the user's home area. */
export async function createHostEntry(
  request: ShellRequest,
  parent: string,
  name: string,
  directory: boolean,
  device: string,
  signal: AbortSignal,
): Promise<void> {
  const query = new URLSearchParams({ area: "home", path: parent, name, directory: String(directory), device });
  const response = await request(`/api/v1/shell/fs?${query.toString()}`, {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    signal,
  });
  if (!response.ok) throw fsFailure(response.status);
}

/** Renames one entry inside the user's home area. */
export async function renameHostEntry(
  request: ShellRequest,
  path: string,
  name: string,
  device: string,
  signal: AbortSignal,
): Promise<void> {
  const query = new URLSearchParams({ area: "home", path, name, device });
  const response = await request(`/api/v1/shell/fs?${query.toString()}`, {
    method: "PUT",
    cache: "no-store",
    credentials: "same-origin",
    signal,
  });
  if (!response.ok) throw fsFailure(response.status === 400 ? 409 : response.status);
}

/** Removes one file or empty directory from the user's home area. */
export async function removeHostEntry(
  request: ShellRequest,
  path: string,
  device: string,
  signal: AbortSignal,
): Promise<void> {
  const query = new URLSearchParams({ area: "home", path, device });
  const response = await request(`/api/v1/shell/fs?${query.toString()}`, {
    method: "DELETE",
    cache: "no-store",
    credentials: "same-origin",
    signal,
  });
  if (!response.ok) throw fsFailure(response.status === 400 ? 409 : response.status);
}

/** Areas the rumahl path syntax addresses. */
export type RumahlArea = "home" | "apps" | "system" | "desktop";

export interface RumahlPath {
  /** Canonical form shown in the address bar. */
  canonical: string;
  area: RumahlArea;
  /** Path relative to the area root, without a leading slash. */
  relative: string;
}

function hasControlCharacter(value: string): boolean {
  for (const character of value) {
    const code = character.charCodeAt(0);
    if (code < 0x20 || code === 0x7f) return true;
  }
  return false;
}

const AREA_ALIASES: Record<string, RumahlArea> = {
  "~": "home",
  home: "home",
  files: "home",
  store: "home",
  apps: "apps",
  "app-data": "apps",
  system: "system",
  desktop: "desktop"
};

/**
 * Parses what a user typed into the Files address bar into a canonical rumahl
 * path. This is deliberately not a POSIX interpreter: there are no drive
 * letters, no absolute host paths and no `..`; backslashes are treated as
 * separators so a path pasted from Windows still lands where it points.
 */
export function parseRumahlPath(input: string): RumahlPath | null {
  let value = input.trim();
  if (value.length >= 2 && ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'")))) {
    value = value.slice(1, -1).trim();
  }
  value = value.replace(/\\/g, "/");
  const lower = value.toLowerCase();
  if (lower.startsWith("rumahl://")) value = value.slice("rumahl://".length);
  else if (lower.startsWith("rumahl:")) value = value.slice("rumahl:".length);
  if (!value || value.startsWith("/")) return null;
  const segments = value.split("/").filter((segment) => segment.length > 0 && segment !== ".");
  if (segments.some((segment) => segment === ".." || segment.length > 255 || hasControlCharacter(segment))) return null;
  const area = AREA_ALIASES[segments[0]?.toLowerCase() ?? ""];
  if (!area) return null;
  const relative = segments.slice(1).join("/");
  if (area === "desktop" && relative) return null;
  if (relative.length > 4096) return null;
  return { canonical: relative ? `rumahl://${area}/${relative}` : `rumahl://${area}`, area, relative };
}

export interface ResolvedRumahlPath extends RumahlPath {
  name: string;
  directory: boolean;
  writable: boolean;
}

export type PathResolutionReason = "invalid" | "denied" | "missing" | "unavailable";

export class PathResolutionError extends Error {
  readonly reason: PathResolutionReason;

  constructor(reason: PathResolutionReason) {
    super(reason);
    this.name = "PathResolutionError";
    this.reason = reason;
  }
}

/**
 * Asks the service to resolve a canonical rumahl path. The service applies the
 * same area gates as browsing (home always, apps/system in advanced mode), so
 * the client never decides whether a path is reachable.
 */
export async function resolveRumahlPath(
  request: ShellRequest,
  path: RumahlPath,
  device: string,
  signal: AbortSignal,
): Promise<ResolvedRumahlPath> {
  const query = new URLSearchParams({ path: path.canonical, device });
  const response = await request(`/api/v1/shell/fs/resolve?${query.toString()}`, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) {
    throw new PathResolutionError(
      response.status === 400 ? "invalid" : response.status === 403 ? "denied" : response.status === 404 ? "missing" : "unavailable",
    );
  }
  const value = record(await response.json());
  if (value.fsVersion !== 1 ||
      (value.area !== "home" && value.area !== "apps" && value.area !== "system" && value.area !== "desktop") ||
      typeof value.path !== "string" || value.path.length > 4096 ||
      typeof value.relative !== "string" || value.relative.length > 4096 ||
      typeof value.name !== "string" || value.name.length > 255 ||
      typeof value.directory !== "boolean" ||
      typeof value.writable !== "boolean")
    throw new PathResolutionError("unavailable");
  return {
    canonical: value.path,
    area: value.area,
    relative: value.relative,
    name: value.name,
    directory: value.directory,
    writable: value.writable,
  };
}
