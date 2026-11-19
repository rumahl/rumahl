import { useSyncExternalStore } from "react";

const KEY = "rumahl.app.disabled";
const EVENT = "rumahl:app-disabled";

function parse(value: string | null): readonly string[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? Array.from(new Set(parsed.filter((id): id is string => typeof id === "string" && /^[A-Za-z0-9._-]{1,255}$/.test(id))))
      : [];
  } catch {
    return [];
  }
}

const EMPTY: readonly string[] = [];
let cached: readonly string[] | null = null;
const listeners = new Set<() => void>();

function read(): readonly string[] {
  if (cached === null) {
    try { cached = parse(localStorage.getItem(KEY)); } catch { cached = []; }
  }
  return cached;
}

function refresh(): void {
  try { cached = parse(localStorage.getItem(KEY)); } catch { cached = []; }
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  const onStorage = (event: StorageEvent) => { if (event.key === KEY) refresh(); };
  window.addEventListener("storage", onStorage);
  window.addEventListener(EVENT, refresh);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", onStorage);
    window.removeEventListener(EVENT, refresh);
  };
}

export function setAppEnabled(id: string, enabled: boolean): void {
  const current = new Set(read());
  if (enabled) current.delete(id); else current.add(id);
  try { localStorage.setItem(KEY, JSON.stringify([...current])); } catch { /* keep session state */ }
  cached = [...current];
  window.dispatchEvent(new Event(EVENT));
}

export function useDisabledApps(): readonly string[] {
  return useSyncExternalStore(subscribe, read, () => EMPTY);
}
