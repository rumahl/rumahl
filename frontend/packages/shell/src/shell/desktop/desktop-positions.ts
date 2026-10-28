import { useSyncExternalStore } from "react";

export interface DesktopPosition { x: number; y: number }

export interface DesktopBounds { width: number; height: number }

/**
 * Clamps a free icon position so the icon (its `footprint`) stays inside the
 * (measured) shortcuts container. Keeps stale or dragged-off-screen positions
 * from hiding a desktop icon or stacking it against the edge.
 */
export function clampDesktopPosition(
  position: DesktopPosition,
  bounds: DesktopBounds,
  footprint: DesktopBounds
): DesktopPosition {
  return {
    x: Math.min(Math.max(0, Math.round(position.x)), Math.max(0, bounds.width - footprint.width)),
    y: Math.min(Math.max(0, Math.round(position.y)), Math.max(0, bounds.height - footprint.height))
  };
}

/** True when the position already lies inside the container. */
export function desktopPositionInBounds(
  position: DesktopPosition,
  bounds: DesktopBounds,
  footprint: DesktopBounds
): boolean {
  const clamped = clampDesktopPosition(position, bounds, footprint);
  return clamped.x === Math.round(position.x) && clamped.y === Math.round(position.y);
}

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

/** Stable empty snapshot for the server render (positions are client-only). */
const SERVER_POSITIONS: Record<string, DesktopPosition> = {};
export function getServerDesktopPositions(): Record<string, DesktopPosition> { return SERVER_POSITIONS; }

export function subscribeDesktopPositions(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function useDesktopPositions(): Record<string, DesktopPosition> {
  return useSyncExternalStore(subscribeDesktopPositions, getDesktopPositions, getServerDesktopPositions);
}
