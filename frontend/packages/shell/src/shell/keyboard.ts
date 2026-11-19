import { useSyncExternalStore } from "react";

/**
 * rumahl OS keyboard commands.
 *
 * The leader key (`r`) opens a dynamic command palette that lists the available
 * commands for the current context, filters them as you type and runs the top
 * match. Real text input is never hijacked: the leader only arms outside input
 * fields, and the emergency secure-attention chord always goes to `/recovery`.
 */
const paletteListeners = new Set<() => void>();
let paletteOpen = false;
function emitPalette() { for (const listener of paletteListeners) listener(); }
export function openKeyPalette(): void { if (!paletteOpen) { paletteOpen = true; emitPalette(); } }
export function closeKeyPalette(): void { if (paletteOpen) { paletteOpen = false; emitPalette(); } }
export function useKeyPaletteOpen(): boolean {
  return useSyncExternalStore(
    (listener) => { paletteListeners.add(listener); return () => { paletteListeners.delete(listener); }; },
    () => paletteOpen,
    () => false
  );
}

function isTextTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return element.tagName === "INPUT" || element.tagName === "TEXTAREA" || element.tagName === "SELECT" || element.isContentEditable;
}

/** The emergency secure-attention chord. */
export function isEmergencyChord(event: KeyboardEvent): boolean {
  const ctrlAlt = event.ctrlKey && event.altKey;
  const deleteKey = event.key === "Delete" || event.key === "Backspace";
  if (ctrlAlt && (deleteKey || event.key.toLowerCase() === "e")) return true;
  return event.ctrlKey && event.shiftKey && event.key === "Escape";
}

export function installShortcuts({ recovery = true }: { recovery?: boolean | undefined } = {}): () => void {
  const onKey = (event: KeyboardEvent) => {
    // The emergency chord always reaches the standalone recovery system.
    if (isEmergencyChord(event)) {
      if (!recovery) return;
      event.preventDefault();
      window.location.assign("/recovery");
      return;
    }
    if (paletteOpen) return; // the palette owns the keyboard while open
    // Never hijack typing: the leader only arms outside text fields.
    if (isTextTarget(event.target)) return;
    if (event.key === "r" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      openKeyPalette();
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
