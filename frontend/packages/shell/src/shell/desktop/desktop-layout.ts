import { useCallback, useMemo } from "react";
import { useWorkspace, type DesktopArrangement } from "../../preferences/Workspace";

export type DesktopLayout = DesktopArrangement;

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
 * Desktop arrangement, stored in the device-scoped workspace so it is rendered
 * on the first paint instead of being applied from localStorage after mount.
 */
export function useDesktopLayout(appIds: readonly string[]): DesktopLayoutController {
  const workspace = useWorkspace();
  const layout = workspace.effective.desktop;
  const persist = useCallback((next: DesktopLayout) => {
    void workspace.save("device", { ...workspace.effective, desktop: next });
  }, [workspace]);
  const ids = useMemo(() => ordered(layout.order, appIds), [layout.order, appIds]);
  const visibleIds = useMemo(() => ids.filter((id) => !layout.hidden.includes(id)), [ids, layout.hidden]);
  const move = useCallback((fromId: string, toId: string) => {
    const list = ordered(layout.order, appIds);
    const from = list.indexOf(fromId), to = list.indexOf(toId);
    if (from < 0 || to < 0 || from === to) return;
    const next = [...list];
    const [moved] = next.splice(from, 1);
    if (moved) next.splice(to, 0, moved);
    persist({ ...layout, order: next });
  }, [layout, appIds, persist]);
  const hide = useCallback((id: string) => {
    if (layout.hidden.includes(id)) return;
    persist({ ...layout, hidden: [...layout.hidden, id] });
  }, [layout, persist]);
  const restore = useCallback((id: string) => {
    if (!layout.hidden.includes(id)) return;
    persist({ ...layout, hidden: layout.hidden.filter((value) => value !== id) });
  }, [layout, persist]);
  const reset = useCallback(() => persist({ order: [], hidden: [], widgets: true }), [persist]);
  const toggleWidgets = useCallback(() => persist({ ...layout, widgets: !layout.widgets }), [layout, persist]);
  return { layout, orderedIds: ids, visibleIds, move, hide, restore, reset, toggleWidgets };
}
