import { createContext, useContext, useEffect, useRef, useState, type PropsWithChildren } from "react";
import type { ShellLiveSource } from "../live-updates";
import { browserProfile } from "./storage";
import { PreferencesHttpError, requestOsMode, type OsMode, type OsModeSettings, type PreferenceScope } from "./client";

/** What a mode exposes. The server stays authoritative; this mirrors it for UI. */
export interface OsModePolicy {
  guided: boolean;
  advancedSettings: boolean;
  browseSystemFiles: boolean;
  terminal: boolean;
  ssh: boolean;
  webConsole: boolean;
}

export function osModePolicy(mode: OsMode): OsModePolicy {
  const developer = mode === "developer";
  const advanced = mode === "advanced" || developer;
  return {
    guided: mode === "guided",
    advancedSettings: advanced,
    browseSystemFiles: advanced,
    terminal: developer,
    ssh: developer,
    webConsole: developer
  };
}

type OsModeError = "unavailable" | "conflict" | "forbidden" | null;

interface OsModeContextValue {
  mode: OsMode;
  policy: OsModePolicy;
  ready: boolean;
  saving: boolean;
  error: OsModeError;
  /** Changes the mode. Raising to advanced/developer requires the account password. */
  change: (scope: PreferenceScope, mode: OsMode | null, password?: string) => Promise<boolean>;
}

const Context = createContext<OsModeContextValue | null>(null);
const guidedSettings: OsModeSettings = { settingsVersion: 1, ownerId: "demo", revision: 0, user: { osMode: "guided" }, device: { osMode: null }, effective: { osMode: "guided" } };

export function OsModeProvider({ live, children }: PropsWithChildren<{ live: ShellLiveSource | undefined }>) {
  // Fail closed: until the server answers, the shell behaves as guided.
  const [settings, setSettings] = useState<OsModeSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<OsModeError>(null);
  const current = useRef<OsModeSettings | null>(null);
  const writer = useRef<OsModeContextValue["change"]>(async () => false);

  useEffect(() => {
    const profile = browserProfile();
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    let reading = false;
    let writing = false;
    let generation = 0;
    function apply(value: OsModeSettings) {
      if (controller.signal.aborted) return;
      current.current = value;
      setSettings(value);
    }
    async function refresh() {
      if (!live || writing || reading || controller.signal.aborted) return;
      reading = true;
      const started = generation;
      try {
        const value = await requestOsMode(live.request, profile.id, controller.signal);
        if (!controller.signal.aborted && started === generation) {
          apply(value);
          setError((previous) => (previous === "unavailable" || previous === "forbidden" ? null : previous));
        }
      } catch {
        if (!controller.signal.aborted && started === generation) setError("unavailable");
      } finally {
        reading = false;
      }
    }
    async function poll() {
      await refresh();
      if (!controller.signal.aborted) timer = setTimeout(() => { void poll(); }, document.hidden ? 15_000 : 5_000);
    }
    if (!live) apply(guidedSettings);
    else void poll();

    writer.current = async (scope, mode, password) => {
      if (!live) {
        const next = mode ?? "guided";
        apply({ ...guidedSettings, effective: { osMode: next } });
        notify();
        return true;
      }
      if (writing || !current.current || controller.signal.aborted) return false;
      writing = true;
      generation += 1;
      setSaving(true);
      setError(null);
      try {
        const value = await requestOsMode(live.request, profile.id, controller.signal, {
          revision: current.current.revision,
          scope,
          mode,
          ...(password ? { password } : {})
        });
        apply(value);
        notify();
        return true;
      } catch (failure) {
        if (controller.signal.aborted) return false;
        if (failure instanceof PreferencesHttpError && failure.status === 409) {
          try { apply(await requestOsMode(live.request, profile.id, controller.signal)); } catch { /* keep last confirmed mode */ }
          setError("conflict");
        } else if (failure instanceof PreferencesHttpError && failure.status === 403) {
          setError("forbidden");
        } else {
          setError("unavailable");
        }
        return false;
      } finally {
        writing = false;
        if (!controller.signal.aborted) setSaving(false);
      }
    };

    const refreshNow = () => { if (!document.hidden) void refresh(); };
    window.addEventListener("focus", refreshNow);
    document.addEventListener("visibilitychange", refreshNow);
    return () => {
      controller.abort();
      clearTimeout(timer);
      writer.current = async () => false;
      window.removeEventListener("focus", refreshNow);
      document.removeEventListener("visibilitychange", refreshNow);
    };
  }, [live]);

  const mode = settings?.effective.osMode ?? "guided";
  return <Context value={{
    mode,
    policy: osModePolicy(mode),
    ready: settings !== null,
    saving,
    error,
    change: (scope, mode, password) => writer.current(scope, mode, password)
  }}>{children}</Context>;
}

function notify() {
  try { window.dispatchEvent(new Event("rumahl:os-mode-changed")); } catch { /* no window */ }
}

export function useOsMode() {
  const value = useContext(Context);
  if (!value) throw new Error("OS mode provider missing");
  return value;
}

export function useOsModePolicy(): OsModePolicy {
  return useOsMode().policy;
}
