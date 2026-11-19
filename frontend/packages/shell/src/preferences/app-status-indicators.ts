import { useSyncExternalStore } from "react";

const KEY = "rumahl.app.statusIndicators";
const EVENT = "rumahl:app-status-indicators";

function read(): boolean {
  try { return localStorage.getItem(KEY) === "true"; } catch { return false; }
}

function subscribe(callback: () => void): () => void {
  const onStorage = (event: StorageEvent) => { if (event.key === KEY) callback(); };
  window.addEventListener(EVENT, callback);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(EVENT, callback);
    window.removeEventListener("storage", onStorage);
  };
}

export function setAppStatusIndicators(enabled: boolean): void {
  try { localStorage.setItem(KEY, enabled ? "true" : "false"); } catch { /* keep session state */ }
  window.dispatchEvent(new Event(EVENT));
}

export function useAppStatusIndicators(): boolean {
  return useSyncExternalStore(subscribe, read, () => false);
}
