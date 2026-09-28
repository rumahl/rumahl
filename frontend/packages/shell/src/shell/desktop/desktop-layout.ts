import { useCallback, useEffect, useMemo, useState } from "react";

export interface DesktopLayout {
  /** User-arranged app order. Unknown ids are ignored, new apps append. */
  order: string[];
  /** App ids the user removed from the desktop. */
  hidden: string[];
  /** Whether the desktop shows clock/status widgets. */
  widgets: boolean;
}

const KEY = "rumahl.desktop.layout.v1";
const EMPTY: DesktopLayout = { order: [], hidden: [], widgets: true };
const MAX = 512;

export function parseDesktopLayout(value: unknown): DesktopLayout {
  const input = (typeof value === "object" && value ? value : {}) as Record<string, unknown>;
  const order = strings(input.order);
  const hidden = strings(input.hidden);
  return { order, hidden, widgets: input.widgets !== false };
}

function strings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const id of value) {
    if (typeof id !== "string" || !id || id.length > 255 || seen.has(id)) continue;
    seen.add(id);
    result.push(id);
    if (result.length >= MAX) break;
  }
  return result;
}

function ordered(order: readonly string[], appIds: readonly string[]): string[] {
  const known = new Set(appIds);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const id of order) if (known.has(id) && !seen.has(id)) { seen.add(id); result.push(id); }
  for (const id of appIds) if (!seen.has(id)) { seen.add(id); result.push(id); }
  return result;
}

export interface DesktopLayoutController {
  layout: DesktopLayout;
  orderedIds: readonly string[];
  visibleIds: readonly string[];
  move: (fromId: string, toId: string) => void;
  hide: (id: string) => void;
  restore: (id: string) => void;
  reset: () => void;
  toggleWidgets: () => void;
}

/**
 * Device-local desktop arrangement. Stored outside the synced workspace so
 * window layout and folder grouping stay account-scoped while icon placement
 * remains a per-screen preference.
 */
export function useDesktopLayout(appIds: readonly string[]): DesktopLayoutController {
  const [layout, setLayout] = useState<DesktopLayout>(EMPTY);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) setLayout(parseDesktopLayout(JSON.parse(raw)));
    } catch {
      // Keep the default arrangement when storage is unavailable or corrupt.
    }
    setLoaded(true);
  }, []);
  useEffect(() => {
    if (!loaded) return;
    try { localStorage.setItem(KEY, JSON.stringify(layout)); } catch { /* session-only arrangement */ }
  }, [layout, loaded]);

  const ids = useMemo(() => ordered(layout.order, appIds), [layout.order, appIds]);
  const visibleIds = useMemo(() => ids.filter((id) => !layout.hidden.includes(id)), [ids, layout.hidden]);
  const move = useCallback((fromId: string, toId: string) => {
    setLayout((previous) => {
      const list = ordered(previous.order, appIds);
      const from = list.indexOf(fromId), to = list.indexOf(toId);
      if (from < 0 || to < 0 || from === to) return previous;
      const next = [...list];
      const [moved] = next.splice(from, 1);
      if (moved) next.splice(to, 0, moved);
      return { ...previous, order: next };
    });
  }, [appIds]);
  const hide = useCallback((id: string) => setLayout((previous) =>
    previous.hidden.includes(id) ? previous : { ...previous, hidden: [...previous.hidden, id] }
  ), []);
  const restore = useCallback((id: string) => setLayout((previous) =>
    previous.hidden.includes(id) ? { ...previous, hidden: previous.hidden.filter((value) => value !== id) } : previous
  ), []);
  const reset = useCallback(() => setLayout(EMPTY), []);
  const toggleWidgets = useCallback(() => setLayout((previous) => ({ ...previous, widgets: !previous.widgets })), []);
  return { layout, orderedIds: ids, visibleIds, move, hide, restore, reset, toggleWidgets };
}
