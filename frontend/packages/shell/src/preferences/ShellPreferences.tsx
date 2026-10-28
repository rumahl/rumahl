import { createContext, useContext, useEffect, useRef, useState, type PropsWithChildren } from "react";
import { DEFAULT_THEME_ID } from "@rumahl/ui/themes";
import type { ShellLiveSource } from "../live-updates";
import { isMode, isThemeId, PreferencesHttpError, requestPreferences, type Preferences, type PreferenceKey, type PreferenceScope, type ShellMode } from "./client";
import { browserProfile, cachePreferences, notifyPreferencesChanged, PREFERENCES_CHANGED } from "./storage";
type PreferenceValue = ShellMode | string | null;
interface SettingsContext {
  scope: PreferenceScope;
  setScope: (scope: PreferenceScope) => void;
  mode: ShellMode;
  theme: string;
  preferences: Preferences | null;
  ready: boolean;
  saving: boolean;
  error: "unavailable" | "conflict" | null;
  persistence: "local" | "session" | "memory";
  save: (scope: PreferenceScope, value: ShellMode | null) => void;
  saveTheme: (scope: PreferenceScope, value: string | null) => void;
}
const Context = createContext<SettingsContext | null>(null);
const demoUser = { shellMode: "desktop" as const, shellTheme: DEFAULT_THEME_ID };
const demoDefaults: Preferences = { settingsVersion: 1, ownerId: "demo", revision: 0, user: demoUser, device: { shellMode: null, shellTheme: null }, effective: demoUser };
export function ShellPreferencesProvider({ live, initial, children }: PropsWithChildren<{
  live: ShellLiveSource | undefined;
  initial?: { mode: ShellMode; theme: string } | undefined;
}>) {
  // SSR and the first client render always agree; storage is accessed only after hydration.
  const [scope, setScope] = useState<PreferenceScope>("user");
  const [preferences, setPreferences] = useState<Preferences | null>(() => initial ? {
    settingsVersion: 1,
    ownerId: "snapshot",
    revision: 0,
    user: { shellMode: initial.mode, shellTheme: initial.theme },
    device: { shellMode: null, shellTheme: null },
    effective: { shellMode: initial.mode, shellTheme: initial.theme }
  } : null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<SettingsContext["error"]>(null);
  const [persistence, setPersistence] = useState<SettingsContext["persistence"]>("memory");
  const current = useRef<Preferences | null>(null);
  const writer = useRef<(scope: PreferenceScope, update: { key: PreferenceKey; value: PreferenceValue }) => void>(() => {});
  useEffect(() => {
    const profile = browserProfile();
    setPersistence(profile.persistence);
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    let reading = false;
    let writing = false;
    let generation = 0;
    function apply(value: Preferences) {
      if (controller.signal.aborted) return;
      if (current.current?.ownerId === value.ownerId && current.current.revision === value.revision &&
          current.current.user.shellMode === value.user.shellMode && current.current.device.shellMode === value.device.shellMode &&
          current.current.user.shellTheme === value.user.shellTheme && current.current.device.shellTheme === value.device.shellTheme) return;
      current.current = value; setPreferences(value); cachePreferences(profile.id, value);
    }
    async function refresh() {
      if (!live || writing || reading || controller.signal.aborted) return;
      reading = true;
      const started = generation;
      try {
        const value = await requestPreferences(live.request, profile.id, controller.signal);
        if (!controller.signal.aborted && started === generation) { apply(value); setError((previous) => previous === "unavailable" ? null : previous); }
      } catch { if (!controller.signal.aborted && started === generation) setError("unavailable"); }
      finally { reading = false; }
    }
    async function poll() {
      await refresh();
      if (!controller.signal.aborted) timer = setTimeout(() => { void poll(); }, document.hidden ? 10_000 : 2_000);
    }
    if (!live) {
      let mode: ShellMode = "desktop";
      let theme = DEFAULT_THEME_ID;
      try {
        const storedMode = localStorage.getItem("rumahl.demo.shell-mode");
        if (isMode(storedMode)) mode = storedMode;
        const storedTheme = localStorage.getItem("rumahl.demo.shell-theme");
        if (isThemeId(storedTheme)) theme = storedTheme;
      } catch { /* demo works without storage */ }
      // The stored demo values act as the account baseline; a browser override is
      // represented by a non-null device value written later.
      const user = { shellMode: mode, shellTheme: theme };
      apply({ ...demoDefaults, user, device: { shellMode: null, shellTheme: null }, effective: user });
    } else void poll();
    writer.current = (scope, update) => {
      if (writing || !current.current || controller.signal.aborted) return;
      if (!live) {
        const previous = current.current;
        let user = previous.user;
        let device = previous.device;
        if (update.key === "shell.mode") {
          if (scope === "user") user = { ...user, shellMode: (update.value as ShellMode | null) ?? "desktop" };
          else device = { ...device, shellMode: update.value as ShellMode | null };
        } else {
          if (scope === "user") user = { ...user, shellTheme: (update.value as string | null) ?? DEFAULT_THEME_ID };
          else device = { ...device, shellTheme: update.value as string | null };
        }
        const next = { ...previous, user, device, effective: { shellMode: device.shellMode ?? user.shellMode, shellTheme: device.shellTheme ?? user.shellTheme } };
        apply(next);
        try {
          localStorage.setItem("rumahl.demo.shell-mode", next.effective.shellMode);
          localStorage.setItem("rumahl.demo.shell-theme", next.effective.shellTheme);
        } catch { /* demo fallback */ }
        return;
      }
      writing = true; generation += 1; setSaving(true); setError(null);
      const revision = current.current.revision;
      void requestPreferences(live.request, profile.id, controller.signal, { revision, scope, key: update.key, value: update.value }).then((value) => {
        apply(value); notifyPreferencesChanged();
      }).catch(async (failure: unknown) => {
        if (controller.signal.aborted) return;
        if (failure instanceof PreferencesHttpError && failure.status === 409) {
          try { apply(await requestPreferences(live.request, profile.id, controller.signal)); } catch { /* keep last confirmed state */ }
          if (!controller.signal.aborted) setError("conflict");
        } else setError("unavailable");
      }).finally(() => { writing = false; if (!controller.signal.aborted) setSaving(false); });
    };
    const refreshNow = () => { void refresh(); };
    const storageChanged = (event: StorageEvent) => { if (event.key === PREFERENCES_CHANGED) refreshNow(); };
    window.addEventListener("focus", refreshNow);
    window.addEventListener("storage", storageChanged);
    document.addEventListener("visibilitychange", refreshNow);
    return () => { controller.abort(); clearTimeout(timer); writer.current = () => {}; window.removeEventListener("focus", refreshNow); window.removeEventListener("storage", storageChanged); document.removeEventListener("visibilitychange", refreshNow); };
  }, [live]);
  return <Context value={{
    scope, setScope,
    mode: preferences?.effective.shellMode ?? "desktop",
    theme: preferences?.effective.shellTheme ?? DEFAULT_THEME_ID,
    preferences, ready: preferences !== null, saving, error, persistence,
    save: (scope, value) => writer.current(scope, { key: "shell.mode", value }),
    saveTheme: (scope, value) => writer.current(scope, { key: "shell.theme", value })
  }}>{children}</Context>;
}
export function useShellPreferences() {
  const value = useContext(Context);
  if (!value) throw new Error("Shell preferences provider missing");
  return value;
}
