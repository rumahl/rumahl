import { useSyncExternalStore } from "react";
import {
  defineTheme,
  listThemes,
  registerTheme,
  type ThemeInput
} from "@rumahl/ui/sdk";
import { themes, type Theme } from "@rumahl/ui/themes";
import { ThemeSafetyError, validateTheme } from "./theme-safety";

/**
 * Custom theme installer.
 *
 * Themes are **data**: installing a theme registers its JSON (tokens + optional
 * stylesheet) in the SDK registry and persists it on the device, so it appears
 * in the theme picker and resolves by id — no code changes. The registry is the
 * same one the OS resolves the active theme from (`getTheme`).
 */
const STORAGE_KEY = "rumahl.installed-themes";

const listeners = new Set<() => void>();
let snapshot: Theme[] = [...themes];
const installedIds = new Set<string>();

function notify(): void {
  snapshot = [...listThemes()];
  listeners.forEach((listener) => listener());
}

function read(): unknown[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(value) ? value : [];
  } catch {
    return [];
  }
}

function write(items: unknown[]): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {
    /* storage may be unavailable; the theme still works for this session */
  }
}

/** Registers every installed theme; call once before the first render. */
export function loadInstalledThemes(): void {
  for (const raw of read()) {
    try {
      if (!validateTheme(raw).ok) continue;
      installedIds.add(registerTheme(defineTheme(raw as ThemeInput)).id);
    } catch {
      /* skip invalid entries */
    }
  }
  notify();
}

/** Installs a theme from its JSON data (safety-checked, registered + persisted). */
export function installTheme(data: unknown): Theme {
  const report = validateTheme(data);
  if (!report.ok) throw new ThemeSafetyError(report);
  const theme = registerTheme(defineTheme(data as ThemeInput));
  const items = read().filter(
    (item) => (item as { id?: string }).id !== theme.id
  );
  items.push(data);
  write(items);
  installedIds.add(theme.id);
  notify();
  return theme;
}

/** Installs a theme from a picked `.json` file. */
export async function installThemeFile(file: File): Promise<Theme> {
  const text = await file.text();
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error("invalid-theme");
  }
  return installTheme(data);
}

/** True when the theme was installed by the user (not built in). */
export function isInstalledTheme(id: string): boolean {
  return installedIds.has(id);
}

export function subscribeThemeRegistry(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getThemeRegistry(): Theme[] {
  return snapshot;
}

/** All themes the OS knows (built-in + installed), reactive to installs. */
export function useThemeRegistry(): Theme[] {
  return useSyncExternalStore(
    subscribeThemeRegistry,
    getThemeRegistry,
    getThemeRegistry
  );
}
