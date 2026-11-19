import { useEffect, useState, type RefObject } from "react";
import { useSyncExternalStore } from "react";

/**
 * Shared free-grid model for the launcher springboard and the desktop.
 *
 * Items live on a responsive grid: a cell coordinate `{ col, row }` (not pixels)
 * is stored, so the layout adapts to the container size — changing the window
 * only changes the number of columns/rows, never the stored arrangement.
 * `resolveCells` guarantees two items never share a cell.
 */
export interface GridCell { col: number; row: number }

/** How cells are filled for items that have no stored cell. */
export type GridFlow = "row" | "column";

export interface GridMetrics { cols: number; rows: number }

/** Column/row count that fits `cellW × cellH` (+ `gap`) into the container. */
export function gridMetrics(width: number, height: number, cellW: number, cellH: number, gap: number): GridMetrics {
  return {
    cols: Math.max(1, Math.floor((width + gap) / (cellW + gap))),
    rows: Math.max(1, Math.floor((height + gap) / (cellH + gap)))
  };
}

function clampCell(cell: GridCell, cols: number, rows: number): GridCell {
  return { col: Math.min(Math.max(0, Math.round(cell.col)), cols - 1), row: Math.min(Math.max(0, Math.round(cell.row)), rows - 1) };
}

/**
 * Assigns a unique, in-bounds cell to every id. Stored cells win (clamped to the
 * grid); on a collision the nearest free cell is used. Unplaced ids fill the
 * remaining cells in `flow` order. Two ids can never end up in the same cell.
 */
export function resolveCells(
  ids: readonly string[],
  stored: Record<string, GridCell>,
  cols: number,
  rows: number,
  flow: GridFlow = "row"
): Record<string, GridCell> {
  const occupied = new Set<string>();
  const result: Record<string, GridCell> = {};
  const key = (c: GridCell) => `${c.col}:${c.row}`;
  const at = (col: number, row: number): GridCell => ({ col, row });
  const order: GridCell[] = [];
  if (flow === "row") for (let row = 0; row < rows; row += 1) for (let col = 0; col < cols; col += 1) order.push(at(col, row));
  else for (let col = 0; col < cols; col += 1) for (let row = 0; row < rows; row += 1) order.push(at(col, row));
  const nextFree = (): GridCell => order.find((cell) => !occupied.has(key(cell))) ?? at(0, 0);
  const nearestFree = (target: GridCell): GridCell => {
    const start = clampCell(target, cols, rows);
    if (!occupied.has(key(start))) return start;
    let best = nextFree();
    let bestDistance = Infinity;
    for (const cell of order) {
      if (occupied.has(key(cell))) continue;
      const distance = Math.abs(cell.col - start.col) + Math.abs(cell.row - start.row);
      if (distance < bestDistance) { best = cell; bestDistance = distance; }
    }
    return best;
  };
  for (const id of ids) {
    const cell = stored[id] ? nearestFree(stored[id]) : nextFree();
    occupied.add(key(cell));
    result[id] = cell;
  }
  return result;
}

/** The nearest free cell to `target` given a set of already-taken cells. */
export function nearestFreeCell(target: GridCell, taken: readonly GridCell[], cols: number, rows: number): GridCell {
  const occupied = new Set(taken.map((cell) => `${cell.col}:${cell.row}`));
  const start = clampCell(target, cols, rows);
  if (!occupied.has(`${start.col}:${start.row}`)) return start;
  let best = start;
  let bestDistance = Infinity;
  for (let col = 0; col < cols; col += 1) for (let row = 0; row < rows; row += 1) {
    const key = `${col}:${row}`;
    if (occupied.has(key)) continue;
    const distance = Math.abs(col - start.col) + Math.abs(row - start.row);
    if (distance < bestDistance) { best = { col, row }; bestDistance = distance; }
  }
  return best;
}

const EMPTY: Record<string, GridCell> = {};

export interface CellStore {
  get: () => Record<string, GridCell>;
  subscribe: (listener: () => void) => () => void;
  use: () => Record<string, GridCell>;
  set: (id: string, cell: GridCell | null) => void;
}

/** A localStorage-backed store of per-item grid cells. */
export function createCellStore(storageKey: string): CellStore {
  const listeners = new Set<() => void>();
  let cells: Record<string, GridCell> = (() => {
    if (typeof localStorage === "undefined") return {};
    try {
      const raw = localStorage.getItem(storageKey);
      const parsed: unknown = raw ? JSON.parse(raw) : {};
      return parsed && typeof parsed === "object" ? parsed as Record<string, GridCell> : {};
    } catch { return {}; }
  })();
  const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
  return {
    get: () => cells,
    subscribe,
    use: () => useSyncExternalStore(subscribe, () => cells, () => EMPTY),
    set: (id, cell) => {
      const next = { ...cells };
      if (cell) next[id] = cell; else delete next[id];
      cells = next;
      if (typeof localStorage !== "undefined") { try { localStorage.setItem(storageKey, JSON.stringify(cells)); } catch { /* ignore */ } }
      for (const listener of listeners) listener();
    }
  };
}

/** Measures an element's content box and keeps it in sync on resize. */
export function useGridSize(ref: RefObject<HTMLElement | null>): { width: number; height: number } {
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const measure = () => setSize({ width: element.clientWidth, height: element.clientHeight });
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return size;
}
