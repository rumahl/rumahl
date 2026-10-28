import type { ShellWindow } from "../shell-state";
import type { WindowPlacement } from "./desktop/geometry";

/**
 * Browser-local recovery snapshot for the live desktop.
 *
 * It is deliberately separate from the server workspace: this captures the
 * windows that are open *right now* (including positions and the active route)
 * so a reload, logout or expired session can be resumed after signing in again.
 * The payload is AES-GCM encrypted; the key lives in IndexedDB when available,
 * with a local-storage fallback for browsers without IndexedDB.
 */

const DB_NAME = "rumahl-session-v1";
const DB_STORE = "keys";
const KEY_ID = "desktop";
const FALLBACK_KEY = "rumahl.session.key.v1";
const STORAGE_KEY = "rumahl.session.snapshot.v1";
const MAX_WINDOWS = 32;
const MAX_AGE = 30 * 24 * 60 * 60 * 1000;
const DISABLED_KEY = "rumahl.session.snapshot.disabled";

function disabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  try { return localStorage.getItem(DISABLED_KEY) === "1"; } catch { return false; }
}

export interface SessionSnapshot {
  version: 1;
  account: string;
  savedAt: number;
  windows: ShellWindow[];
  focusedWindowId: string | null;
  activeLocation: string | null;
}

interface Envelope {
  version: 1;
  iv: string;
  data: string;
}

let keyPromise: Promise<CryptoKey | null> | null = null;
let latest: SessionSnapshot | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let writes: Promise<boolean> = Promise.resolve(true);

function bytesToBase64(bytes: Uint8Array): string {
  let text = "";
  for (const byte of bytes) text += String.fromCharCode(byte);
  return btoa(text);
}

function base64ToBytes(value: string): Uint8Array<ArrayBuffer> {
  const text = atob(value);
  const buffer = new ArrayBuffer(text.length);
  const bytes = new Uint8Array(buffer);
  for (let index = 0; index < text.length; index += 1) bytes[index] = text.charCodeAt(index);
  return bytes;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(DB_STORE)) request.result.createObjectStore(DB_STORE);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function readIndexedKey(): Promise<CryptoKey | null> {
  if (typeof indexedDB === "undefined" || !globalThis.crypto?.subtle) return null;
  const db = await openDatabase();
  try {
    return await new Promise((resolve, reject) => {
      const request = db.transaction(DB_STORE, "readonly").objectStore(DB_STORE).get(KEY_ID);
      request.onsuccess = () => resolve(request.result instanceof CryptoKey ? request.result : null);
      request.onerror = () => reject(request.error);
    });
  } finally {
    db.close();
  }
}

async function writeIndexedKey(key: CryptoKey): Promise<void> {
  const db = await openDatabase();
  try {
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(DB_STORE, "readwrite");
      transaction.objectStore(DB_STORE).put(key, KEY_ID);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
  } finally {
    db.close();
  }
}

async function fallbackKey(): Promise<CryptoKey | null> {
  if (typeof localStorage === "undefined" || !globalThis.crypto?.subtle) return null;
  const existing = localStorage.getItem(FALLBACK_KEY);
  if (existing) {
    return crypto.subtle.importKey("raw", base64ToBytes(existing), "AES-GCM", false, ["encrypt", "decrypt"]);
  }
  const raw = crypto.getRandomValues(new Uint8Array(32));
  localStorage.setItem(FALLBACK_KEY, bytesToBase64(raw));
  return crypto.subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt", "decrypt"]);
}

async function getKey(): Promise<CryptoKey | null> {
  if (!globalThis.crypto?.subtle) return null;
  if (!keyPromise) keyPromise = (async () => {
    try {
      const existing = await readIndexedKey();
      if (existing) return existing;
      const key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
      await writeIndexedKey(key);
      return key;
    } catch {
      try { return await fallbackKey(); } catch { return null; }
    }
  })();
  return keyPromise;
}

function validRect(value: unknown): value is ShellWindow["rect"] {
  if (!value || typeof value !== "object") return false;
  const rect = value as Record<string, unknown>;
  return ["x", "y", "width", "height"].every((key) => typeof rect[key] === "number" && Number.isFinite(rect[key]))
    && Number(rect.width) >= 1 && Number(rect.height) >= 1;
}

function validWindow(value: unknown): value is ShellWindow {
  if (!value || typeof value !== "object") return false;
  const window = value as Record<string, unknown>;
  if (typeof window.id !== "string" || !window.id || window.id.length > 255
    || typeof window.title !== "string" || window.title.length > 512
    || typeof window.subtitle !== "string" || window.subtitle.length > 512
    || typeof window.minimized !== "boolean"
    || (window.location !== undefined && (typeof window.location !== "string" || window.location.length > 2048 || /[\r\n\\]/.test(window.location)))
    || (window.rect !== undefined && !validRect(window.rect))
    || (window.placement !== undefined && !["floating", "maximized", "left", "right"].includes(window.placement as string))
    || (window.fullscreen !== undefined && typeof window.fullscreen !== "boolean")
    || (window.streamId !== undefined && (typeof window.streamId !== "string" || window.streamId.length > 128))
    || (window.preferredSize !== undefined && (!window.preferredSize || typeof window.preferredSize !== "object"))
  ) return false;
  return true;
}

export function normalizeSessionSnapshot(value: unknown, account: string): SessionSnapshot | null {
  if (!value || typeof value !== "object") return null;
  const snapshot = value as Record<string, unknown>;
  if (snapshot.version !== 1 || snapshot.account !== account || typeof snapshot.savedAt !== "number"
    || !Number.isFinite(snapshot.savedAt) || Date.now() - snapshot.savedAt > MAX_AGE
    || !Array.isArray(snapshot.windows) || snapshot.windows.length > MAX_WINDOWS
    || !snapshot.windows.every(validWindow)
  ) return null;
  const focusedWindowId = typeof snapshot.focusedWindowId === "string" ? snapshot.focusedWindowId : null;
  const activeLocation = typeof snapshot.activeLocation === "string"
    && snapshot.activeLocation.length <= 2048
    && !/[\r\n\\]/.test(snapshot.activeLocation)
    ? snapshot.activeLocation
    : null;
  return {
    version: 1,
    account,
    savedAt: snapshot.savedAt,
    windows: snapshot.windows as ShellWindow[],
    focusedWindowId,
    activeLocation,
  };
}

export function createSessionSnapshot(
  account: string,
  windows: readonly ShellWindow[],
  focusedWindowId: string | null,
  activeLocation: string | null,
): SessionSnapshot {
  return {
    version: 1,
    account,
    savedAt: Date.now(),
    windows: windows
      .filter((window) => window.location !== undefined)
      .slice(0, MAX_WINDOWS)
      .map((window) => ({
        id: window.id,
        title: window.title,
        subtitle: window.subtitle,
        minimized: window.minimized,
        ...(window.location ? { location: window.location } : {}),
        ...(window.streamId ? { streamId: window.streamId } : {}),
        ...(window.rect ? { rect: window.rect } : {}),
        ...(window.placement ? { placement: window.placement as WindowPlacement } : {}),
        ...(window.fullscreen ? { fullscreen: true } : {}),
        ...(window.preferredSize ? { preferredSize: window.preferredSize } : {}),
      })),
    focusedWindowId,
    activeLocation,
  };
}

async function writeSnapshot(snapshot: SessionSnapshot): Promise<boolean> {
  const key = await getKey();
  if (!key || typeof localStorage === "undefined") return false;
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const plaintext = new TextEncoder().encode(JSON.stringify(snapshot));
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, plaintext);
  const envelope: Envelope = { version: 1, iv: bytesToBase64(iv), data: bytesToBase64(new Uint8Array(ciphertext)) };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(envelope));
  return true;
}

/** Queues the latest live desktop; coalesces rapid window/drag updates. */
export function stageSessionSnapshot(snapshot: SessionSnapshot): void {
  if (disabled()) return;
  latest = snapshot;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    const pending = latest;
    if (pending) writes = writes.then(() => writeSnapshot(pending)).catch(() => false);
  }, 350);
}

/** Encrypts the latest staged state immediately (session expiry, logout, pagehide). */
export function flushSessionSnapshot(): Promise<boolean> {
  if (disabled()) return Promise.resolve(true);
  if (timer) { clearTimeout(timer); timer = null; }
  const pending = latest;
  if (pending) writes = writes.then(() => writeSnapshot(pending)).catch(() => false);
  return writes;
}

export function hasStagedSessionSnapshot(): boolean {
  return latest !== null;
}

export async function loadSessionSnapshot(account: string): Promise<SessionSnapshot | null> {
  if (typeof localStorage === "undefined" || disabled()) return null;
  let envelope: Envelope;
  try { envelope = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") as Envelope; } catch { return null; }
  if (!envelope || envelope.version !== 1 || typeof envelope.iv !== "string" || typeof envelope.data !== "string") return null;
  const key = await getKey();
  if (!key) return null;
  try {
    const plaintext = await crypto.subtle.decrypt({ name: "AES-GCM", iv: base64ToBytes(envelope.iv) }, key, base64ToBytes(envelope.data));
    return normalizeSessionSnapshot(JSON.parse(new TextDecoder().decode(plaintext)), account);
  } catch {
    return null;
  }
}

export function clearSessionSnapshot(): void {
  latest = null;
  if (timer) { clearTimeout(timer); timer = null; }
  try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
}
