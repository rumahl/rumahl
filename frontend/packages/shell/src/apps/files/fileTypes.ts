import { useCallback, useSyncExternalStore } from "react";

/**
 * File-type associations: which program opens a file, decided by its extension.
 *
 * This is intentionally a small, swappable registry. Extensions map to a
 * handler id that names an installed program (a viewer/editor). A file with an
 * unknown extension, or a program that is not installed, is never downloaded as
 * a side effect of opening it.
 */

export type FileHandlerKind = "viewer" | "editor";

export interface FileHandler {
  id: string;
  /** Name of the program that opens the type. */
  label: string;
  kind: FileHandlerKind;
  /** True once a viewer/editor program is actually available in the OS. */
  installed: boolean;
  /** App route to launch once the program is installed. */
  appPath?: string;
}

/** Programs a file type can be opened with. More are added as apps arrive. */
export const fileHandlers: readonly FileHandler[] = [
  { id: "text", label: "Text editor", kind: "editor", installed: true, appPath: "/app/editor" },
  { id: "image", label: "Image viewer", kind: "viewer", installed: false },
  { id: "media", label: "Media player", kind: "viewer", installed: false },
  { id: "pdf", label: "PDF viewer", kind: "viewer", installed: false }
];

export function handlerById(id: string | undefined): FileHandler | undefined {
  return id ? fileHandlers.find((handler) => handler.id === id) : undefined;
}

/**
 * Default extension → handler id. The extension is the key precisely so the
 * program behind it can be swapped later without changing this table's shape.
 */
export const defaultAssociations: Readonly<Record<string, string>> = {
  txt: "text", text: "text", md: "text", markdown: "text", mdx: "text", rst: "text", adoc: "text",
  json: "text", jsonc: "text", json5: "text", csv: "text", tsv: "text", properties: "text",
  ts: "text", tsx: "text", js: "text", jsx: "text", mjs: "text", cjs: "text", vue: "text", svelte: "text",
  yml: "text", yaml: "text", toml: "text", ini: "text", conf: "text", cfg: "text", env: "text", log: "text",
  xml: "text", html: "text", htm: "text", css: "text", scss: "text", less: "text",
  sh: "text", bash: "text", zsh: "text", fish: "text", rs: "text", py: "text", rb: "text", go: "text",
  java: "text", kt: "text", swift: "text", c: "text", h: "text", cpp: "text", hpp: "text", cs: "text",
  php: "text", sql: "text", graphql: "text", gql: "text", diff: "text", patch: "text", tex: "text",
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", svg: "image",
  bmp: "image", avif: "image", heic: "image",
  mp3: "media", wav: "media", flac: "media", ogg: "media", m4a: "media", opus: "media",
  mp4: "media", mkv: "media", webm: "media", mov: "media", avi: "media",
  pdf: "pdf"
};

/** Well-known extensionless text files (dotfiles and plain names). */
const TEXT_DOTFILES = new Set(["gitignore", "gitattributes", "editorconfig", "env", "npmrc", "prettierrc", "eslintrc", "dockerignore"]);
const TEXT_PLAIN_NAMES = new Set(["makefile", "dockerfile", "license", "licence", "readme", "changelog", "procfile", "justfile"]);

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

/** Resolve the registered program for a file name, honouring overrides. */
export function fileAssociation(name: string, overrides: Overrides = load()): FileHandler | undefined {
  const extension = extensionOf(name);
  if (extension) return handlerById(overrides[extension]) ?? handlerById(defaultAssociations[extension]);
  const trimmed = name.trim().toLowerCase();
  const dotfile = /^\.[a-z0-9_-]{1,64}$/.exec(trimmed)?.[0].slice(1);
  if ((dotfile && TEXT_DOTFILES.has(dotfile)) || TEXT_PLAIN_NAMES.has(trimmed)) {
    return handlerById(overrides[dotfile ?? trimmed]) ?? handlerById("text");
  }
  return undefined;
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
