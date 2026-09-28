import { useEffect, useState } from "react";
import type { ShellApp } from "./useShellApps";

const KEY = "rumahl.app.recents.v1";
const LIMIT = 6;

/** Maps a stored location back to the app that owns it (longest path wins). */
export function resolveRecentApp(path: string, apps: readonly ShellApp[]): ShellApp | undefined {
  return apps
    .filter((app) => path === app.path || path.startsWith(`${app.path}/`))
    .sort((a, b) => b.path.length - a.path.length)[0];
}

/** Small, device-local list of recently opened shell locations. */
export function readRecents(): string[] {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string =>
          typeof value === "string" && value.startsWith("/") && !value.startsWith("//") && !value.includes("\\")
        ).slice(0, LIMIT)
      : [];
  } catch {
    return [];
  }
}

export function rememberRecent(path: string): void {
  if (!path.startsWith("/") || path.startsWith("//") || path.includes("\\")) return;
  try {
    const next = [path, ...readRecents().filter((value) => value !== path)].slice(0, LIMIT);
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // Restricted storage: recents are a convenience, not required for the shell.
  }
}

export function useRecents(): readonly string[] {
  const [paths, setPaths] = useState<readonly string[]>([]);
  useEffect(() => {
    setPaths(readRecents());
    const update = () => setPaths(readRecents());
    window.addEventListener("storage", update);
    window.addEventListener("focus", update);
    return () => {
      window.removeEventListener("storage", update);
      window.removeEventListener("focus", update);
    };
  }, []);
  return paths;
}
