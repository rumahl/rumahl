import { describe, expect, it } from "vitest";
import { DEMO_THEME_SOURCES, seedDemoThemes, type DemoThemeStorage } from "./seed-themes";

function memoryStorage(initial: Record<string, string> = {}): DemoThemeStorage & { dump: () => Record<string, string> } {
  const store = new Map(Object.entries(initial));
  return {
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => void store.set(key, value),
    dump: () => Object.fromEntries(store)
  };
}

const THEMES_KEY = "rumahl.installed-themes";
const SEED_KEY = "rumahl.demo.seeded-themes.v1";

describe("demo theme seeding", () => {
  it("ships one real, non-native theme package", () => {
    const packages = DEMO_THEME_SOURCES.map((source) => JSON.parse(source) as { id: string; base?: string });
    expect(packages.map((entry) => entry.id)).toEqual(["com.example.meridian"]);
    expect(packages[0]?.base).toBe("ground");
  });

  it("installs the theme once and marks it as seeded", () => {
    const storage = memoryStorage();
    seedDemoThemes(storage);

    const installed = JSON.parse(storage.dump()[THEMES_KEY] ?? "[]") as { id: string; origin: string }[];
    expect(installed).toHaveLength(1);
    expect(installed[0]?.id).toBe("com.example.meridian");
    expect(installed[0]?.origin).toBe("manual");
    expect(JSON.parse(storage.dump()[SEED_KEY] ?? "[]")).toEqual(["com.example.meridian"]);
  });

  it("does not duplicate an already installed theme", () => {
    const storage = memoryStorage();
    seedDemoThemes(storage);
    seedDemoThemes(storage);
    expect(JSON.parse(storage.dump()[THEMES_KEY] ?? "[]")).toHaveLength(1);
  });

  it("keeps themes the visitor installed themselves", () => {
    const storage = memoryStorage({
      [THEMES_KEY]: JSON.stringify([{ id: "com.example.other", name: "Other" }])
    });
    seedDemoThemes(storage);
    const ids = (JSON.parse(storage.dump()[THEMES_KEY] ?? "[]") as { id: string }[]).map((entry) => entry.id);
    expect(ids).toContain("com.example.other");
    expect(ids).toContain("com.example.meridian");
  });

  it("does not resurrect a theme the visitor removed", () => {
    const storage = memoryStorage();
    seedDemoThemes(storage);
    storage.setItem(THEMES_KEY, JSON.stringify([]));
    seedDemoThemes(storage);
    expect(JSON.parse(storage.dump()[THEMES_KEY] ?? "[]")).toEqual([]);
  });

  it("survives corrupt storage without throwing", () => {
    const storage = memoryStorage({ [THEMES_KEY]: "{", [SEED_KEY]: "null" });
    expect(() => seedDemoThemes(storage)).not.toThrow();
    const installed = JSON.parse(storage.dump()[THEMES_KEY] ?? "[]") as { id: string }[];
    expect(installed.map((entry) => entry.id)).toEqual(["com.example.meridian"]);
  });
});
