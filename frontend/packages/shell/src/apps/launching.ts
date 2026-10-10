import { useSyncExternalStore } from "react";

/**
 * Which installed apps are currently launching. The dock reads this to bounce
 * the app icon (like macOS) while the sandboxed iframe is still loading.
 */
const launching = new Set<string>();
const subscribers = new Set<() => void>();
let version = 0;

function subscribe(callback: () => void): () => void {
  subscribers.add(callback);
  return () => subscribers.delete(callback);
}

export function setLaunching(appId: string, value: boolean): void {
  const changed = value ? !launching.has(appId) : launching.has(appId);
  if (!changed) return;
  if (value) launching.add(appId);
  else launching.delete(appId);
  version += 1;
  for (const subscriber of subscribers) subscriber();
}

export function isLaunching(appId: string): boolean {
  return launching.has(appId);
}

/** Re-renders the caller whenever any launch state changes. */
export function useLaunching(): number {
  return useSyncExternalStore(subscribe, () => version, () => 0);
}
