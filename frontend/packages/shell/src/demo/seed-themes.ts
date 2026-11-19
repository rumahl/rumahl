import meridianSource from "../../themes/meridian.json?raw";

/**
 * Demo-only theme seeding.
 *
 * The demo ships one real theme package so the public demo already has an
 * installed, non-native theme to try. The package is the same JSON a visitor
 * can install through the theme manager; nothing here is a built-in theme.
 *
 * Production never imports this module; only `src/demo/main.tsx` seeds.
 */
const THEME_STORAGE_KEY = "rumahl.installed-themes";
const SEED_STORAGE_KEY = "rumahl.demo.seeded-themes.v1";

export const DEMO_THEME_SOURCES: readonly string[] = [meridianSource];

export interface DemoThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function parseArray(value: string | null): unknown[] {
  try {
    const parsed = JSON.parse(value ?? "[]");
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function parsePackage(source: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(source) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    const id = (parsed as { id?: unknown }).id;
    return typeof id === "string" && id.trim() ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

/**
 * Installs the demo theme packages once. A theme the visitor removed is not
 * resurrected: the marker records what was already offered, so the demo only
 * fills the gap on a fresh browser profile.
 */
export function seedDemoThemes(
  storage: DemoThemeStorage | undefined = typeof localStorage === "undefined" ? undefined : localStorage
): void {
  if (!storage) return;
  try {
    const seeded = new Set(
      parseArray(storage.getItem(SEED_STORAGE_KEY)).filter((id): id is string => typeof id === "string")
    );
    const packages = DEMO_THEME_SOURCES.map(parsePackage).filter(
      (entry): entry is Record<string, unknown> => entry !== null
    );
    const pending = packages.filter((entry) => !seeded.has(entry.id as string));
    if (pending.length === 0) return;

    const items = parseArray(storage.getItem(THEME_STORAGE_KEY));
    for (const entry of pending) {
      const record = { ...entry, origin: "manual" };
      const index = items.findIndex((item) => (item as { id?: unknown }).id === entry.id);
      if (index >= 0) items[index] = record;
      else items.push(record);
      seeded.add(entry.id as string);
    }
    storage.setItem(SEED_STORAGE_KEY, JSON.stringify([...seeded]));
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify(items));
  } catch {
    /* storage unavailable: the demo still runs without the seeded theme */
  }
}
