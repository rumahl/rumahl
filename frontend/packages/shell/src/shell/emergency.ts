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

/**
 * Secure attention sequence: Ctrl+Alt+Delete / Ctrl+Alt+E, or Ctrl+Shift+Escape
 * (the Windows task-manager shortcut). Escape closes the emergency shell.
 */
export function emergencyShortcut(event: KeyboardEvent): boolean {
  const ctrlAlt = event.ctrlKey && event.altKey;
  const deleteKey = event.key === "Delete" || event.key === "Backspace";
  if (ctrlAlt && (deleteKey || event.key.toLowerCase() === "e")) return true;
  return event.ctrlKey && event.shiftKey && event.key === "Escape";
}

export function installEmergencyShortcut(): () => void {
  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") { closeEmergency(); return; }
    if (emergencyShortcut(event)) {
      event.preventDefault();
      openEmergency();
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
