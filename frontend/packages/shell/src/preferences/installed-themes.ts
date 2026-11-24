import { useSyncExternalStore } from "react";
import {
  defineTheme,
  listThemes,
  registerTheme,
  resetThemes,
  type ThemeInput
} from "@rumahl/ui/sdk";
import { themes, type Theme } from "@rumahl/ui/themes";
import { ThemeSafetyError, validateTheme, type SafetyReport } from "./theme-safety";

/**
 * Custom theme installer.
 *
 * Themes are **data**: installing a theme registers its JSON (tokens + optional
 * stylesheet) in the SDK registry and persists it on the device, so it appears
 * in the theme picker and resolves by id — no code changes. The registry is the
 * same one the OS resolves the active theme from (`getTheme`).
 */
const STORAGE_KEY = "rumahl.installed-themes";

const listeners = new Set<() => void>();
let snapshot: Theme[] = [...themes];
const installedIds = new Set<string>();

function notify(): void {
  snapshot = [...listThemes()];
  listeners.forEach((listener) => listener());
}

function read(): unknown[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(value) ? value : [];
  } catch {
    return [];
  }
}

function write(items: unknown[]): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {
    /* storage may be unavailable; the theme still works for this session */
  }
}

/**
 * Hash used for code-change detection. Browsers use SHA-256 via WebCrypto;
 * the deterministic fallback only exists for test environments without it.
 */
export function fallbackCodeHash(code: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < code.length; index += 1) {
    hash ^= code.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `fnv1a-${hash.toString(16).padStart(8, "0")}`;
}

export async function themeCodeHash(code: string): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return fallbackCodeHash(code);
  const digest = await subtle.digest("SHA-256", new TextEncoder().encode(code));
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function originOf(record: Record<string, unknown>): "store" | "manual" {
  return record.origin === "store" ? "store" : "manual";
}

/** Registers one persisted record with its local execution decision. */
function registerRecord(raw: unknown): Theme | null {
  try {
    const record = raw as Record<string, unknown>;
    if (!validateTheme(raw).ok) return null;
    const defined = defineTheme(raw as ThemeInput);
    const codeHash = typeof record.codeHash === "string" ? record.codeHash : undefined;
    const trustedHash = typeof record.trustedHash === "string" ? record.trustedHash : undefined;
    const execution = defined.code
      ? record.execution === "trusted" && codeHash && trustedHash === codeHash
        ? "trusted" as const
        : "sandbox" as const
      : undefined;
    return registerTheme({
      ...defined,
      origin: originOf(record),
      ...(execution ? { execution } : {}),
      ...(codeHash ? { codeHash } : {}),
      ...(trustedHash ? { trustedHash } : {}),
    });
  } catch {
    return null;
  }
}

/** Registers every installed theme; call once before the first render. */
export function loadInstalledThemes(): void {
  for (const raw of read()) {
    const theme = registerRecord(raw);
    if (theme) installedIds.add(theme.id);
  }
  notify();
}

export interface ThemeInstallOptions {
  /** Sandbox is the default; trusted is only valid with a matching code hash. */
  execution?: "sandbox" | "trusted" | undefined;
  codeHash?: string | undefined;
}

/** Installs a theme from its JSON data (safety-checked, registered + persisted). */
export function installTheme(
  data: unknown,
  origin: "store" | "manual" = "manual",
  options: ThemeInstallOptions = {}
): Theme {
  const report = validateTheme(data);
  if (!report.ok) throw new ThemeSafetyError(report);
  const defined = defineTheme(data as ThemeInput);
  const codeHash = defined.code ? options.codeHash : undefined;
  const execution = defined.code
    ? options.execution === "trusted" && codeHash
      ? "trusted" as const
      : "sandbox" as const
    : undefined;
  const trustedHash = execution === "trusted" ? codeHash : undefined;
  const theme = registerTheme({
    ...defined,
    origin,
    ...(execution ? { execution } : {}),
    ...(codeHash ? { codeHash } : {}),
    ...(trustedHash ? { trustedHash } : {}),
  });
  const items = read().filter(
    (item) => (item as { id?: string }).id !== theme.id
  );
  items.push({
    ...(data as Record<string, unknown>),
    origin,
    ...(execution ? { execution } : {}),
    ...(codeHash ? { codeHash } : {}),
    ...(trustedHash ? { trustedHash } : {}),
  });
  write(items);
  installedIds.add(theme.id);
  notify();
  return theme;
}

/** True when the installed code matches the explicitly trusted hash. */
export function isThemeTrusted(theme: Theme): boolean {
  return theme.execution === "trusted"
    && typeof theme.codeHash === "string"
    && theme.trustedHash === theme.codeHash;
}

/**
 * Marks the installed code as trusted for full shell access. The hash is
 * recomputed from the current code if the caller does not pass one, so a
 * theme update invalidates the consent until the user trusts the new code.
 */
export async function trustTheme(id: string, codeHash?: string): Promise<void> {
  const items = read();
  const index = items.findIndex((item) => (item as { id?: string }).id === id);
  if (index < 0) throw new Error("theme not installed");
  const record = items[index] as Record<string, unknown>;
  const defined = defineTheme(record as unknown as ThemeInput);
  if (!defined.code) return;
  const hash = codeHash
    ?? (typeof record.codeHash === "string" ? record.codeHash : undefined)
    ?? await themeCodeHash(defined.code);
  const trusted = { ...defined, origin: originOf(record), execution: "trusted" as const, codeHash: hash, trustedHash: hash };
  items[index] = { ...record, execution: "trusted", codeHash: hash, trustedHash: hash };
  write(items);
  registerTheme(trusted);
  notify();
}

/** Revokes full shell access; the theme keeps its CSS and installed code. */
export function revokeThemeTrust(id: string): void {
  const items = read();
  const index = items.findIndex((item) => (item as { id?: string }).id === id);
  if (index < 0) return;
  const record = items[index] as Record<string, unknown>;
  const defined = defineTheme(record as unknown as ThemeInput);
  if (!defined.code) return;
  const codeHash = typeof record.codeHash === "string" ? record.codeHash : undefined;
  const next: Record<string, unknown> = { ...record, execution: "sandbox" };
  delete next.trustedHash;
  items[index] = next;
  write(items);
  registerTheme({
    ...defined,
    origin: originOf(record),
    execution: "sandbox",
    ...(codeHash ? { codeHash } : {}),
  });
  notify();
}

/** Reads a ZIP archive (stored or deflated entries) into a file map. */
async function readZipEntries(buffer: ArrayBuffer): Promise<Map<string, Uint8Array>> {
  const view = new DataView(buffer);
  let end = -1;
  for (let offset = buffer.byteLength - 22; offset >= Math.max(0, buffer.byteLength - 65557); offset -= 1) {
    if (view.getUint32(offset, true) === 0x06054b50) {
      end = offset;
      break;
    }
  }
  if (end < 0) throw new Error("invalid-theme");
  const count = view.getUint16(end + 10, true);
  let cursor = view.getUint32(end + 16, true);
  const files = new Map<string, Uint8Array>();
  const decoder = new TextDecoder();
  for (let index = 0; index < count; index += 1) {
    if (view.getUint32(cursor, true) !== 0x02014b50) break;
    const method = view.getUint16(cursor + 10, true);
    const compressed = view.getUint32(cursor + 20, true);
    const nameLength = view.getUint16(cursor + 28, true);
    const extraLength = view.getUint16(cursor + 30, true);
    const commentLength = view.getUint16(cursor + 32, true);
    const local = view.getUint32(cursor + 42, true);
    const name = decoder.decode(new Uint8Array(buffer, cursor + 46, nameLength));
    const localName = view.getUint16(local + 26, true);
    const localExtra = view.getUint16(local + 28, true);
    const start = local + 30 + localName + localExtra;
    const raw = new Uint8Array(buffer, start, compressed);
    if (method === 0) {
      files.set(name, raw);
    } else if (method === 8) {
      const stream = new Blob([raw]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      files.set(name, new Uint8Array(await new Response(stream).arrayBuffer()));
    } else {
      throw new Error("invalid-theme");
    }
    cursor += 46 + nameLength + extraLength + commentLength;
  }
  return files;
}

/**
 * A theme package is a ZIP with `theme.json` at its root (or one directory
 * deep). `css` and `code` may name sibling files; their contents are inlined so
 * the installed theme stays a single data record.
 */
async function readThemePackage(buffer: ArrayBuffer): Promise<unknown> {
  const files = await readZipEntries(buffer);
  const decoder = new TextDecoder();
  const entry = [...files.entries()].find(([name]) => /(^|\/)theme\.json$/i.test(name));
  if (!entry) throw new Error("invalid-theme");
  const [name, bytes] = entry;
  const directory = name.slice(0, name.length - "theme.json".length);
  const text = (data: Uint8Array | undefined) => (data ? decoder.decode(data) : undefined);
  const inline = (value: unknown, extension: string) => {
    if (typeof value !== "string") return value;
    // Already a stylesheet/module body.
    if (value.includes("\n") || value.length > 400 || !value.toLowerCase().endsWith(extension)) return value;
    return text(files.get(directory + value) ?? files.get(value)) ?? value;
  };
  let manifest: Record<string, unknown>;
  try {
    manifest = JSON.parse(text(bytes) ?? "null") as Record<string, unknown>;
  } catch {
    throw new Error("invalid-theme");
  }
  if (!manifest || typeof manifest !== "object") throw new Error("invalid-theme");
  const preview = manifest.preview;
  if (typeof preview === "string" && /\.(png|jpe?g|webp|avif)$/i.test(preview)) {
    const bytes = files.get(directory + preview) ?? files.get(preview);
    if (bytes) {
      let binary = "";
      for (const byte of bytes) binary += String.fromCharCode(byte);
      const mime = /jpe?g$/i.test(preview) ? "jpeg" : preview.split(".").pop()?.toLowerCase() ?? "png";
      manifest.preview = `data:image/${mime};base64,${btoa(binary)}`;
    }
  }
  return { ...manifest, css: inline(manifest.css, ".css"), code: inline(manifest.code, ".js") };
}

export interface PreparedThemeInstall {
  data: unknown;
  theme: Theme;
  report: SafetyReport;
  /** SHA-256 of the code (or a test fallback when WebCrypto is unavailable). */
  codeHash: string | null;
}

/**
 * Reads and validates a theme package without installing it, so the UI can ask
 * how executable code should run before anything is persisted.
 */
export async function prepareThemeFile(file: File): Promise<PreparedThemeInstall> {
  const head = new Uint8Array(await file.slice(0, 4).arrayBuffer());
  const zip = head[0] === 0x50 && head[1] === 0x4b;
  let data: unknown;
  try {
    data = zip ? await readThemePackage(await file.arrayBuffer()) : JSON.parse(await file.text());
  } catch (error) {
    if (error instanceof Error && error.message === "invalid-theme") throw error;
    throw new Error("invalid-theme", { cause: error });
  }
  const report = validateTheme(data);
  if (!report.ok) throw new ThemeSafetyError(report);
  const theme = defineTheme(data as ThemeInput);
  const codeHash = theme.code ? await themeCodeHash(theme.code) : null;
  return { data, theme, report, codeHash };
}

/** Installs a theme from a picked `.json` or `.zip` package. */
export async function installThemeFile(
  file: File,
  options: ThemeInstallOptions = {}
): Promise<Theme> {
  const prepared = await prepareThemeFile(file);
  return installTheme(prepared.data, "manual", {
    execution: options.execution ?? "sandbox",
    ...(prepared.codeHash ? { codeHash: prepared.codeHash } : {}),
  });
}

/** Removes an installed theme; built-in themes cannot be uninstalled. */
export function uninstallTheme(id: string): void {
  if (!installedIds.has(id)) return;
  const items = read().filter((item) => (item as { id?: string }).id !== id);
  write(items);
  installedIds.clear();
  resetThemes();
  for (const raw of items) {
    const theme = registerRecord(raw);
    if (theme) installedIds.add(theme.id);
  }
  notify();
}

/** Serializes a theme as a portable package (`theme.json` with inline files). */
export function exportThemePackage(theme: Theme): string {
  return JSON.stringify(
    {
      id: theme.id,
      name: theme.name,
      version: "1.0.0",
      ...(theme.schemaVersion ? { schemaVersion: theme.schemaVersion } : {}),
      locked: theme.locked === true,
      modes: theme.modes,
      variants: theme.variants,
      tokens: theme.tokens,
      parameters: theme.parameters,
      ...(theme.capabilities?.length ? { capabilities: theme.capabilities } : {}),
      ...(theme.preview ? { preview: theme.preview } : {}),
      ...(theme.icons && Object.keys(theme.icons).length ? { icons: theme.icons } : {}),
      ...(theme.css ? { css: theme.css } : {}),
      ...(theme.code ? { code: theme.code, ...(theme.runtime ? { runtime: theme.runtime } : {}) } : {})
    },
    null,
    2
  );
}

/** True when the theme was installed by the user (not built in). */
export function isInstalledTheme(id: string): boolean {
  return installedIds.has(id);
}

export function subscribeThemeRegistry(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getThemeRegistry(): Theme[] {
  return snapshot;
}

/** All themes the OS knows (built-in + installed), reactive to installs. */
export function useThemeRegistry(): Theme[] {
  return useSyncExternalStore(
    subscribeThemeRegistry,
    getThemeRegistry,
    getThemeRegistry
  );
}
