import type { Preferences } from "./client";
const PROFILE = "rumahl.browser-profile.v1";
export const PREFERENCES_CHANGED = "rumahl.preferences.changed.v1";
let memoryProfile: string | undefined;
/** No credentials in storage. This random identifier only selects an owned settings namespace. */
export function browserProfile(): { id: string; persistence: "local" | "session" | "memory" } {
  for (const persistence of ["local", "session"] as const) {
    try {
      const storage = persistence === "local" ? window.localStorage : window.sessionStorage;
      const existing = storage.getItem(PROFILE);
      const id = existing && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(existing) ? existing : crypto.randomUUID();
      storage.setItem(PROFILE, id);
      return { id, persistence };
    } catch { /* Restricted browsers retain a session or in-memory profile. */ }
  }
  memoryProfile ??= crypto.randomUUID();
  return { id: memoryProfile, persistence: "memory" };
}
export function cachePreferences(profile: string, value: Preferences): void {
  // Never read a previous user's cache before the server identifies the current account.
  try { localStorage.setItem(`rumahl.preferences.v1:${value.ownerId}:${profile}`, JSON.stringify(value)); }
  catch { try { sessionStorage.setItem(`rumahl.preferences.v1:${value.ownerId}:${profile}`, JSON.stringify(value)); } catch { /* UI continues without durable cache. */ } }
}
export function notifyPreferencesChanged(): void {
  try { localStorage.setItem(PREFERENCES_CHANGED, crypto.randomUUID()); } catch { /* Periodic sync remains active. */ }
}
