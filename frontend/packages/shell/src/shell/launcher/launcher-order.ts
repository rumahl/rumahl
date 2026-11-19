import { useSyncExternalStore } from "react";

/**
 * Springboard order for the launcher: the user can drag app icons freely and
 * across pages. Only the *order* is stored (not pixel positions), so the grid
 * stays responsive — pages are (re)computed from the measured grid size.
 */
const KEY = "rumahl.launcher.order";
const listeners = new Set<() => void>();

function load(): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) && parsed.every((value) => typeof value === "string") ? parsed : [];
  } catch {
    return [];
  }
}

let order: string[] = load();

function emit() { for (const listener of listeners) listener(); }

export function getLauncherOrder(): string[] { return order; }

/** Stable empty snapshot for the server render (order is client-only). */
const SERVER_ORDER: string[] = [];
export function getServerLauncherOrder(): string[] { return SERVER_ORDER; }

export function setLauncherOrder(next: readonly string[]): void {
  order = [...next];
  if (typeof localStorage !== "undefined") {
    try { localStorage.setItem(KEY, JSON.stringify(order)); } catch { /* ignore */ }
  }
  emit();
}

export function subscribeLauncherOrder(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function useLauncherOrder(): string[] {
  return useSyncExternalStore(subscribeLauncherOrder, getLauncherOrder, getServerLauncherOrder);
}

/** Move the item at `from` to index `to`, clamped to the list bounds. */
export function moveInOrder(list: readonly string[], from: number, to: number): string[] {
  const next = [...list];
  if (from < 0 || from >= next.length) return next;
  const clamped = Math.max(0, Math.min(to, next.length - 1));
  const [moved] = next.splice(from, 1);
  if (moved !== undefined) next.splice(clamped, 0, moved);
  return next;
}

/** Merge the known app ids with the stored order, keeping new apps at the end. */
export function applyOrder(ids: readonly string[], stored: readonly string[]): string[] {
  const known = new Set(ids);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const id of stored) if (known.has(id) && !seen.has(id)) { seen.add(id); result.push(id); }
  for (const id of ids) if (!seen.has(id)) { seen.add(id); result.push(id); }
  return result;
}

/** Split an ordered id list into pages of `pageSize`, always returning >= 1 page. */
export function paginate(ids: readonly string[], pageSize: number): string[][] {
  const size = Math.max(1, Math.floor(pageSize));
  const pages: string[][] = [];
  for (let index = 0; index < ids.length; index += size) pages.push(ids.slice(index, index + size));
  return pages.length ? pages : [[]];
}
