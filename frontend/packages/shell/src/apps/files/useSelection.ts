import { useCallback, useRef, useState } from "react";
import type { Item } from "./types";

export interface SelectionEvent {
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

/** Click, ctrl/meta toggle and shift-range selection over the visible order. */
export function useSelection(entries: readonly Item[]) {
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set());
  const anchor = useRef<string | null>(null);

  const clear = useCallback(() => setSelected(new Set()), []);

  const select = useCallback((item: Item, event: SelectionEvent) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (event.shiftKey && anchor.current) {
        const keys = entries.map((entry) => entry.key);
        const from = keys.indexOf(anchor.current);
        const to = keys.indexOf(item.key);
        if (from >= 0 && to >= 0) {
          for (let i = Math.min(from, to); i <= Math.max(from, to); i += 1) next.add(keys[i]!);
          return next;
        }
      }
      if (event.ctrlKey || event.metaKey) {
        if (next.has(item.key)) next.delete(item.key);
        else next.add(item.key);
      } else {
        next.clear();
        next.add(item.key);
      }
      anchor.current = item.key;
      return next;
    });
  }, [entries]);

  const selectAll = useCallback(() => setSelected(new Set(entries.map((entry) => entry.key))), [entries]);

  const setSelection = useCallback((keys: Iterable<string>) => {
    const allowed = new Set(entries.map((entry) => entry.key));
    setSelected(new Set([...keys].filter((key) => allowed.has(key))));
  }, [entries]);

  const prune = useCallback((keys: readonly string[]) => {
    setSelected((previous) => {
      const allowed = new Set(keys);
      const next = new Set([...previous].filter((key) => allowed.has(key)));
      return next.size === previous.size ? previous : next;
    });
  }, []);

  return { selected, select, clear, selectAll, prune, setSelection };
}
