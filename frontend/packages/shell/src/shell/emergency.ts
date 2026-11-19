import { useSyncExternalStore } from "react";

/**
 * The emergency shell (the OS "secure attention" screen). Opened by the secure
 * attention sequence or from the UI; rendered above everything else.
 */
let open = false;
const listeners = new Set<() => void>();
const emit = () => { for (const listener of listeners) listener(); };

export function openEmergency(): void {
  if (!open) { open = true; emit(); }
}
export function closeEmergency(): void {
  if (open) { open = false; emit(); }
}
export function useEmergencyOpen(): boolean {
  return useSyncExternalStore(
    (listener) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    () => open,
    () => false
  );
}
