import { useSyncExternalStore } from "react";

export interface DesktopPosition { x: number; y: number }

const KEY = "rumahl.desktop.positions";
const listeners = new Set<() => void>();

function load(): Record<string, DesktopPosition> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" ? parsed as Record<string, DesktopPosition> : {};
  } catch {
    return {};
  }
}

let positions: Record<string, DesktopPosition> = load();

function emit() { for (const listener of listeners) listener(); }

/** Stores (or clears) a free desktop position for an app icon. */
export function setDesktopPosition(id: string, position: DesktopPosition | null): void {
  const next = { ...positions };
  if (position) next[id] = position; else delete next[id];
  positions = next;
  if (typeof localStorage !== "undefined") {
    try { localStorage.setItem(KEY, JSON.stringify(positions)); } catch { /* ignore */ }
  }
  emit();
}

export function getDesktopPositions(): Record<string, DesktopPosition> { return positions; }
export function subscribeDesktopPositions(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function useDesktopPositions(): Record<string, DesktopPosition> {
  return useSyncExternalStore(subscribeDesktopPositions, getDesktopPositions, getDesktopPositions);
}
