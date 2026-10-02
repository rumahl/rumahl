import { useEffect, useState, useSyncExternalStore } from "react";

/**
 * Debug mode, enable-able only from the browser console:
 *   window.__rumahlDebug.enable()
 * It exposes diagnostics and can trigger shell toasts (used for the
 * performance hint and other debug surfaces).
 */
const KEY = "rumahl.debug";
const listeners = new Set<() => void>();

function read(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(KEY) === "1";
}
let enabled = read();
function emit() { for (const listener of listeners) listener(); }

export function setDebug(value: boolean): void {
  enabled = value;
  if (typeof localStorage !== "undefined") {
    try { localStorage.setItem(KEY, value ? "1" : "0"); } catch { /* ignore */ }
  }
  emit();
}
export function isDebug(): boolean { return enabled; }
function subscribe(listener: () => void): () => void { listeners.add(listener); return () => { listeners.delete(listener); }; }
export function useDebug(): boolean {
  // Gate on mount so the persisted flag never causes a hydration mismatch
  // (`getServerSnapshot` is always false).
  const value = useSyncExternalStore(subscribe, isDebug, () => false);
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  return mounted && value;
}

let toastHandler: ((message: string) => void) | null = null;
export function onDebugToast(handler: (message: string) => void): () => void {
  toastHandler = handler;
  return () => { if (toastHandler === handler) toastHandler = null; };
}
export function debugToast(message?: string): void {
  toastHandler?.(message ?? "rumahl OS — debug toast");
}
