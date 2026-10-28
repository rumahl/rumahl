import { useCallback, useSyncExternalStore } from "react";

/**
 * File-type associations: which program opens a file, decided by its extension.
 *
 * This is intentionally a small, swappable registry. Extensions map to a
 * handler id that names a program (a viewer/editor). Programs are not installed
 * yet, so the default handler is a download; once viewer apps exist they are
 * launched through the same lookup, and the mapping can be changed per extension
 * (see `setFileAssociation`) without touching the Files app.
 */

export type FileHandlerKind = "download" | "viewer" | "editor";

export interface FileHandler {
  id: string;
  /** Name of the program that opens the type. */
  label: string;
  kind: FileHandlerKind;
}

/** The fallback: files without an associated program are downloaded. */
export const downloadHandler: FileHandler = { id: "download", label: "Download", kind: "download" };

/** Programs a file type can be opened with. More are added as apps arrive. */
export const fileHandlers: readonly FileHandler[] = [
  downloadHandler,
  { id: "text", label: "Text editor", kind: "editor" },
  { id: "image", label: "Image viewer", kind: "viewer" },
  { id: "media", label: "Media player", kind: "viewer" },
  { id: "pdf", label: "PDF viewer", kind: "viewer" }
];

export function handlerById(id: string | undefined): FileHandler | undefined {
  return id ? fileHandlers.find((handler) => handler.id === id) : undefined;
}

/**
 * Default extension → handler id. The extension is the key precisely so the
 * program behind it can be swapped later without changing this table's shape.
 */
export const defaultAssociations: Readonly<Record<string, string>> = {
  txt: "text", text: "text", md: "text", markdown: "text", json: "text", csv: "text",
  ts: "text", tsx: "text", js: "text", jsx: "text", mjs: "text", cjs: "text",
  yml: "text", yaml: "text", toml: "text", ini: "text", conf: "text", log: "text",
  xml: "text", html: "text", htm: "text", css: "text", sh: "text", rs: "text", py: "text",
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", svg: "image",
  bmp: "image", avif: "image", heic: "image",
  mp3: "media", wav: "media", flac: "media", ogg: "media", m4a: "media", opus: "media",
  mp4: "media", mkv: "media", webm: "media", mov: "media", avi: "media",
  pdf: "pdf"
};

/** The lowercase extension of a file name, or "" when it has none. */
export function extensionOf(name: string): string {
  const base = name.trim().replace(/^\.+/, "");
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) return "";
  return base.slice(dot + 1).toLowerCase();
}

const STORAGE_KEY = "rumahl.files.associations";

type Overrides = Record<string, string>;

let cache: Overrides | null = null;
const subscribers = new Set<() => void>();

function load(): Overrides {
  if (cache) return cache;
  try {
    const raw = typeof window === "undefined" ? null : window.localStorage.getItem(STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    cache = parsed && typeof parsed === "object" ? (parsed as Overrides) : {};
  } catch {
    cache = {};
  }
  return cache;
}

function persist(next: Overrides): void {
  cache = next;
  try {
    if (typeof window !== "undefined") window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // storage is best-effort; the in-memory override still applies
  }
  for (const notify of subscribers) notify();
}

/** Resolve the program for a file name, honouring per-extension overrides. */
export function fileAssociation(name: string, overrides: Overrides = load()): FileHandler {
  const extension = extensionOf(name);
  return (
    handlerById(overrides[extension]) ??
    handlerById(defaultAssociations[extension]) ??
    downloadHandler
  );
}

/** Change (or clear) the program that opens a given extension. */
export function setFileAssociation(extension: string, handlerId: string | null): void {
  const key = extension.trim().toLowerCase().replace(/^\.+/, "");
  if (!key || (handlerId !== null && !handlerById(handlerId))) return;
  const next = { ...load() };
  if (handlerId) next[key] = handlerId;
  else delete next[key];
  persist(next);
}

export function clearFileAssociation(extension: string): void {
  setFileAssociation(extension, null);
}

export function resetFileAssociations(): void {
  persist({});
}

/** Reactive view for the UI: resolves handlers and lets the user swap them. */
export function useFileAssociations() {
  const overrides = useSyncExternalStore(
    (notify) => {
      subscribers.add(notify);
      return () => {
        subscribers.delete(notify);
      };
    },
    () => load(),
    () => ({}) as Overrides
  );
  const resolve = useCallback((name: string) => fileAssociation(name, overrides), [overrides]);
  return {
    resolve,
    overrides,
    handlers: fileHandlers,
    setAssociation: setFileAssociation,
    clearAssociation: clearFileAssociation,
    reset: resetFileAssociations
  };
}
