import { useSyncExternalStore } from "react";

function colorSchemeQuery(): MediaQueryList | null {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;
}

function subscribe(onChange: () => void): () => void {
  const query = colorSchemeQuery();
  if (!query) return () => {};
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

function getSnapshot(): "light" | "dark" {
  return colorSchemeQuery()?.matches ? "dark" : "light";
}

/** Server and hydration must start from the same deterministic value. */
function getServerSnapshot(): "light" | "dark" {
  return "light";
}

/**
 * Resolves the OS light/dark preference. Client-only rendering reads the media
 * query immediately; SSR hydration starts light and updates after mount.
 */
export function useSystemMode(): "light" | "dark" {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
