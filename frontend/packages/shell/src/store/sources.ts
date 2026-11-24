import type { ShellRequest } from "../snapshot-client";
import { useCallback, useEffect, useMemo, useState } from "react";

export interface StoreSource {
  /** Stable identity: the canonical index URL. */
  id: string;
  /** "github" sources derive their index URL from a repository spec. */
  kind: "website" | "github";
  name: string;
  url: string;
  enabled: boolean;
}

const STORAGE_KEY = "rumahl.store.sources.v1";
const STORE_CHANGED = "rumahl:store-sources";
const MAX_SOURCES = 16;

function normalizeSourceUrl(value: string): string {
  let url: URL;
  try { url = new URL(value.trim()); } catch { throw new Error("invalid source URL"); }
  if (url.protocol !== "https:" || !url.hostname || url.username || url.password || url.hash)
    throw new Error("source URL must be HTTPS without credentials or fragments");
  if (url.href.length > 2048) throw new Error("source URL is too long");
  return url.href;
}

function normalizeName(value: string, url: string): string {
  const name = value.trim();
  if (name.length > 64) throw new Error("source name is too long");
  return name || new URL(url).hostname;
}

const DEFAULT_GITHUB_RAW_BASE = "https://raw.githubusercontent.com";
const GITHUB_OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9_.-]{0,62})$/;
const GITHUB_REPO = /^[A-Za-z0-9_.-]{1,100}$/;
const GITHUB_REF = /^[A-Za-z0-9._-]{1,100}$/;

/** Raw host for GitHub sources. A developer shell may set
 * `globalThis.__RUMAHL_GITHUB_RAW_BASE__` (HTTPS) to point the resolution at a
 * local example-store mirror instead of raw.githubusercontent.com. */
function githubRawBase(): string {
  const override = (globalThis as { __RUMAHL_GITHUB_RAW_BASE__?: unknown }).__RUMAHL_GITHUB_RAW_BASE__;
  if (typeof override === "string" && override.trim()) {
    try {
      const url = new URL(override.trim());
      if (url.protocol === "https:" && !url.username && !url.password && !url.search && !url.hash)
        return url.href.replace(/\/+$/, "");
    } catch { /* fall back to the public host */ }
  }
  return DEFAULT_GITHUB_RAW_BASE;
}

export interface GithubSourceSpec {
  owner: string;
  repo: string;
  /** Branch or tag; path separators are not supported in refs here. */
  ref: string;
  /** Optional directory inside the repository that holds the store. */
  path: string;
  /** Derived raw index URL (`<dir>/store.json`). */
  indexUrl: string;
}

/** Accepts `github:owner/repo[@ref][/path]` and
 * `https://github.com/owner/repo[/tree/ref/path]`. */
export function parseGithubSource(input: string): GithubSourceSpec | null {
  const value = input.trim();
  let owner: string;
  let repo: string;
  let ref = "main";
  let path = "";
  if (value.startsWith("github:")) {
    let spec = value.slice("github:".length).replace(/^\/+|\/+$/g, "");
    const at = spec.indexOf("@");
    if (at >= 0) {
      const refAndPath = spec.slice(at + 1).split("/");
      ref = refAndPath[0] ?? "";
      path = refAndPath.slice(1).join("/");
      spec = spec.slice(0, at);
    }
    const segments = spec.split("/");
    if (segments.length < 2) return null;
    owner = segments[0] ?? "";
    repo = (segments[1] ?? "").replace(/\.git$/, "");
    if (segments.length > 2) path = segments.slice(2).join("/");
  } else if (/^https?:\/\//i.test(value)) {
    let url: URL;
    try { url = new URL(value); } catch { return null; }
    if (url.protocol !== "https:" || !["github.com", "www.github.com"].includes(url.hostname)) return null;
    const segments = url.pathname.split("/").filter(Boolean);
    if (segments.length < 2) return null;
    owner = segments[0] ?? "";
    repo = (segments[1] ?? "").replace(/\.git$/, "");
    const tree = segments[2];
    if (tree === "tree" && segments[3]) {
      ref = segments[3];
      path = segments.slice(4).join("/");
    } else if (segments.length > 2) {
      path = segments.slice(2).join("/");
    }
  } else {
    return null;
  }
  if (!GITHUB_OWNER.test(owner) || !GITHUB_REPO.test(repo) || !GITHUB_REF.test(ref)) return null;
  if (path.split("/").some((segment) => segment === "." || segment === "..")) return null;
  path = path.replace(/^\/+|\/+$/g, "");
  const segments = [owner, repo, ref, ...path.split("/").filter(Boolean), "store.json"];
  const indexUrl = `${githubRawBase()}/${segments.map((segment) => encodeURIComponent(segment)).join("/")}`;
  return { owner, repo, ref, path, indexUrl };
}

function defaultSourceName(url: string, github: GithubSourceSpec | null): string {
  return github ? `${github.owner}/${github.repo}` : new URL(url).hostname;
}

function parseSources(value: unknown): StoreSource[] {
  if (!Array.isArray(value)) return [];
  const sources: StoreSource[] = [];
  for (const item of value.slice(0, MAX_SOURCES)) {
    if (!item || typeof item !== "object") continue;
    const record = item as Record<string, unknown>;
    if (typeof record.url !== "string" || typeof record.name !== "string" || typeof record.enabled !== "boolean") continue;
    try {
      const url = normalizeSourceUrl(record.url);
      const kind = record.kind === "github" ? "github" as const : "website" as const;
      sources.push({ id: url, kind, name: normalizeName(record.name, url), url, enabled: record.enabled });
    } catch { /* ignore invalid persisted entries */ }
  }
  return sources;
}

function read(): StoreSource[] {
  if (typeof localStorage === "undefined") return [];
  try { return parseSources(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]")); } catch { return []; }
}

function write(sources: StoreSource[]): void {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(sources)); } catch { /* private mode */ }
  if (typeof window !== "undefined") window.dispatchEvent(new Event(STORE_CHANGED));
}

export function useStoreSources() {
  const [sources, setSources] = useState<StoreSource[]>(read);
  useEffect(() => {
    const sync = () => setSources(read());
    window.addEventListener(STORE_CHANGED, sync);
    window.addEventListener("storage", sync);
    return () => { window.removeEventListener(STORE_CHANGED, sync); window.removeEventListener("storage", sync); };
  }, []);
  const add = useCallback((name: string, rawUrl: string) => {
    const github = parseGithubSource(rawUrl);
    const url = github ? normalizeSourceUrl(github.indexUrl) : normalizeSourceUrl(rawUrl);
    const kind = github ? "github" as const : "website" as const;
    const trimmed = name.trim();
    if (trimmed.length > 64) throw new Error("source name is too long");
    const current = read();
    if (current.some((source) => source.url === url)) throw new Error("source already exists");
    if (current.length >= MAX_SOURCES) throw new Error("too many sources");
    const next: StoreSource[] = [...current, {
      id: url,
      kind,
      name: trimmed || defaultSourceName(url, github),
      url,
      enabled: true
    }];
    write(next);
    setSources(next);
    return next.at(-1)!;
  }, []);
  const remove = useCallback((id: string) => {
    const next = read().filter((source) => source.id !== id);
    write(next);
    setSources(next);
  }, []);
  const setEnabled = useCallback((id: string, enabled: boolean) => {
    const next = read().map((source) => source.id === id ? { ...source, enabled } : source);
    write(next);
    setSources(next);
  }, []);
  return useMemo(() => ({ sources, add, remove, setEnabled }), [sources, add, remove, setEnabled]);
}

export interface StoreAppEntry {
  sourceId: string;
  sourceName: string;
  id: string;
  title: string;
  version: string;
  summary: string;
  description?: string;
  releaseNotes?: string;
  publisher: string;
  packageUrl: string;
  categories: readonly string[];
  iconUrl: string | null;
  screenshots: readonly string[];
  rating: number | null;
  downloads: number | null;
  featured: boolean;
  /** Human-readable permissions declared by the source metadata. */
  permissions: readonly string[];
}

export interface ParsedStoreIndex {
  name: string;
  apps: StoreAppEntry[];
}

const APP_ID = /^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/;
const MAX_MEDIA_URL = 2048;

function text(value: unknown, max: number): string {
  if (typeof value !== "string" || !value.trim() || value.length > max) throw new Error("invalid store text");
  return value.trim();
}

/** Media must use the same HTTPS origin as the index so the trusted source
 * connection and the store certificate policy also cover images. */
function mediaUrl(value: unknown, base: string): string | null {
  if (typeof value !== "string" || !value.trim() || value.length > MAX_MEDIA_URL) return null;
  try {
    const url = new URL(value, base);
    const source = new URL(base);
    if (url.protocol !== "https:" || url.origin !== source.origin || url.username || url.password || url.hash) return null;
    return url.href;
  } catch {
    return null;
  }
}

function categories(app: Record<string, unknown>): string[] {
  const result: string[] = [];
  const add = (value: unknown) => {
    if (typeof value !== "string") return;
    const name = value.trim();
    if (!name || name.length > 48 || result.length >= 6 || result.includes(name)) return;
    result.push(name);
  };
  add(app.category);
  if (Array.isArray(app.categories)) app.categories.slice(0, 6).forEach(add);
  return result;
}

export function storeImageUrl(url: string): string {
  return `/api/v1/shell/store/image?url=${encodeURIComponent(url)}`;
}

export function parseStoreIndex(value: unknown, source: StoreSource): ParsedStoreIndex {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("invalid store index");
  const record = value as Record<string, unknown>;
  if (record.storeVersion !== 1 || !Array.isArray(record.apps) || record.apps.length > 256)
    throw new Error("invalid store index");
  const name = text(record.name, 96);
  const seen = new Set<string>();
  const apps = record.apps.map((item): StoreAppEntry => {
    if (!item || typeof item !== "object" || Array.isArray(item)) throw new Error("invalid store app");
    const app = item as Record<string, unknown>;
    const id = text(app.id, 255);
    if (!APP_ID.test(id) || seen.has(id)) throw new Error("invalid store app id");
    seen.add(id);
    const rawPackage = text(app.packageUrl, 2048);
    let packageUrl: URL;
    try { packageUrl = new URL(rawPackage, source.url); } catch { throw new Error("invalid package URL"); }
    if (packageUrl.protocol !== "https:" || packageUrl.username || packageUrl.password || packageUrl.hash)
      throw new Error("invalid package URL");
    const screenshots = Array.isArray(app.screenshots)
      ? app.screenshots.slice(0, 6).map((value) => mediaUrl(value, source.url)).filter((value): value is string => value !== null)
      : [];
    const rating = typeof app.rating === "number" && Number.isFinite(app.rating) && app.rating >= 0 && app.rating <= 5
      ? Math.round(app.rating * 10) / 10
      : null;
    const downloads = typeof app.downloads === "number" && Number.isSafeInteger(app.downloads) && app.downloads >= 0
      ? app.downloads
      : null;
    const permissions = Array.isArray(app.permissions)
      ? Array.from(new Set(app.permissions
          .filter((value): value is string => typeof value === "string")
          .map((value) => value.trim())
          .filter((value) => value.length > 0 && value.length <= 120)))
          .slice(0, 16)
      : [];
    return {
      sourceId: source.id,
      sourceName: source.name,
      id,
      title: text(app.title, 256),
      version: text(app.version, 128),
      summary: typeof app.summary === "string" ? app.summary.slice(0, 600) : "",
      description: typeof app.description === "string" ? app.description.slice(0, 12000).trim() : "",
      releaseNotes: typeof app.releaseNotes === "string" ? app.releaseNotes.slice(0, 6000).trim() : "",
      publisher: typeof app.publisher === "string" ? app.publisher.slice(0, 256) : "",
      packageUrl: packageUrl.href,
      categories: categories(app),
      iconUrl: mediaUrl(app.iconUrl, source.url),
      screenshots,
      rating,
      downloads,
      featured: app.featured === true,
      permissions
    };
  });
  return { name, apps };
}

export interface StorePackageFile {
  path: string;
  content: string;
}

/** The source returned 404; a GitHub source falls back to a single app. */
export class StoreSourceNotFoundError extends Error {
  constructor(message = "store source not found") {
    super(message);
    this.name = "StoreSourceNotFoundError";
  }
}

export class UntrustedCertificateError extends Error {
  readonly host: string;
  readonly fingerprint: string;
  constructor(host: string, fingerprint: string) {
    super("store certificate is not trusted");
    this.name = "UntrustedCertificateError";
    this.host = host;
    this.fingerprint = fingerprint;
  }
}

export function formatCertificateFingerprint(value: string): string {
  return value.toUpperCase().match(/.{1,2}/g)?.join(":") ?? value.toUpperCase();
}

export function parseStorePackage(value: unknown): StorePackageFile[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("invalid store package");
  const record = value as Record<string, unknown>;
  if (record.packageVersion !== 1 || !Array.isArray(record.files) || record.files.length === 0 || record.files.length > 2048)
    throw new Error("invalid store package");
  const seen = new Set<string>();
  return record.files.map((item): StorePackageFile => {
    if (!item || typeof item !== "object" || Array.isArray(item)) throw new Error("invalid store package file");
    const file = item as Record<string, unknown>;
    if (typeof file.path !== "string" || !file.path || file.path.length > 1024 ||
        file.path.split("/").some((segment) => !segment || segment === "." || segment === "..") ||
        seen.has(file.path)) throw new Error("invalid store package path");
    if (typeof file.content !== "string" || file.content.length > 44 * 1024 * 1024 ||
        !/^[A-Za-z0-9+/]*={0,2}$/.test(file.content)) throw new Error("invalid store package content");
    seen.add(file.path);
    return { path: file.path, content: file.content };
  });
}

export async function fetchStoreDocument(request: ShellRequest, url: string, signal: AbortSignal, maximum = 2 * 1024 * 1024): Promise<unknown> {
  const response = await request(`/api/v1/shell/store/fetch?url=${encodeURIComponent(url)}`, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal
  });
  const body = await response.text();
  if (response.status === 428) {
    try {
      const value = JSON.parse(body) as Record<string, unknown>;
      if (value.storeError === "untrusted-certificate" &&
          typeof value.host === "string" && value.host.length > 0 && value.host.length <= 253 &&
          typeof value.fingerprint === "string" && /^[0-9a-f]{64}$/.test(value.fingerprint)) {
        throw new UntrustedCertificateError(value.host, value.fingerprint);
      }
    } catch (failure) {
      if (failure instanceof UntrustedCertificateError) throw failure;
    }
    throw new Error("store source unavailable");
  }
  if (response.status === 404) {
    try {
      const value = JSON.parse(body) as Record<string, unknown>;
      if (value.storeError === "not-found") throw new StoreSourceNotFoundError();
    } catch (failure) {
      if (failure instanceof StoreSourceNotFoundError) throw failure;
    }
    throw new Error("store source unavailable");
  }
  if (!response.ok) throw new Error("store source unavailable");
  if (body.length > maximum) throw new Error("store response too large");
  try { return JSON.parse(body); } catch { throw new Error("store returned invalid JSON"); }
}

export async function trustStoreCertificate(request: ShellRequest, host: string, fingerprint: string, signal: AbortSignal): Promise<void> {
  const response = await request("/api/v1/shell/store/trust", {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ host, fingerprint }),
    signal
  });
  if (!response.ok) throw new Error("store certificate could not be trusted");
}

export async function fetchStoreIndex(request: ShellRequest, source: StoreSource, signal: AbortSignal): Promise<ParsedStoreIndex> {
  try {
    return parseStoreIndex(await fetchStoreDocument(request, source.url, signal), source);
  } catch (failure) {
    if (source.kind !== "github" || !(failure instanceof StoreSourceNotFoundError)) throw failure;
    return fetchSingleAppIndex(request, source, signal);
  }
}

/** A GitHub repository without `store.json` may be one signed app: its
 * `manifest.json` names it, `store-package.json` is the installable bundle. */
async function fetchSingleAppIndex(request: ShellRequest, source: StoreSource, signal: AbortSignal): Promise<ParsedStoreIndex> {
  const directory = source.url.slice(0, source.url.lastIndexOf("/") + 1);
  const manifest = await fetchStoreDocument(request, `${directory}manifest.json`, signal) as Record<string, unknown>;
  if (!manifest || typeof manifest !== "object" || manifest.formatVersion !== 1) throw new Error("invalid store source");
  const app = manifest.app as Record<string, unknown> | undefined;
  if (!app || typeof app !== "object") throw new Error("invalid store source");
  const id = app.appId;
  const version = app.version;
  const title = app.displayName;
  if (typeof id !== "string" || !APP_ID.test(id) ||
      typeof version !== "string" || !version.trim() ||
      typeof title !== "string" || !title.trim()) throw new Error("invalid store source");
  return {
    name: source.name,
    apps: [{
      sourceId: source.id,
      sourceName: source.name,
      id,
      title: title.trim(),
      version: version.trim(),
      summary: "",
      publisher: typeof manifest.publisherId === "string" ? manifest.publisherId.slice(0, 256) : "",
      packageUrl: `${directory}store-package.json`,
      categories: [],
      iconUrl: null,
      screenshots: [],
      rating: null,
      downloads: null,
      featured: false,
      permissions: []
    }]
  };
}

export async function fetchStorePackage(request: ShellRequest, entry: StoreAppEntry, signal: AbortSignal): Promise<StorePackageFile[]> {
  return parseStorePackage(await fetchStoreDocument(request, entry.packageUrl, signal, 44 * 1024 * 1024));
}
