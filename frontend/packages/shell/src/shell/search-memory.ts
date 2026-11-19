/** Browser-local shortcuts, never cached targets, titles, queries or permissions. */
export interface SearchMemory { favorites: string[]; recent: string[] }
export const emptySearchMemory = (): SearchMemory => ({ favorites: [], recent: [] });
const LIMIT = 40;

export function parseSearchMemory(value: string | null): SearchMemory {
  try {
    const parsed: unknown = JSON.parse(value ?? "null");
    if (!parsed || typeof parsed !== "object") return emptySearchMemory();
    const data = parsed as Record<string, unknown>;
    const ids = (value: unknown) => Array.isArray(value)
      ? [...new Set(value.filter((id): id is string => typeof id === "string" && id.length > 0 && id.length < 256 && id !== "typed" && id !== "design.rl"))].slice(0, LIMIT)
      : [];
    return { favorites: ids(data.favorites), recent: ids(data.recent) };
  } catch { return emptySearchMemory(); }
}

export function rememberSearch(memory: SearchMemory, id: string): SearchMemory {
  if (id === "typed" || id === "design.rl") return memory;
  return { ...memory, recent: [id, ...memory.recent.filter(value => value !== id)].slice(0, LIMIT) };
}

export function toggleSearchFavorite(memory: SearchMemory, id: string): SearchMemory {
  if (id === "typed" || id === "design.rl") return memory;
  return { ...memory, favorites: memory.favorites.includes(id)
    ? memory.favorites.filter(value => value !== id)
    : [...memory.favorites, id].slice(-LIMIT) };
}

/** Resolve every stored id against the currently available, enabled entries. */
export function resolveSearchMemory<T extends { id: string; disabled: boolean }>(ids: string[], entries: T[]): T[] {
  return ids.flatMap(id => {
    const entry = entries.find(item => item.id === id && !item.disabled);
    return entry ? [entry] : [];
  });
}
