import { useSyncExternalStore } from "react";
import { openEmergency } from "./emergency";

/**
 * rumahl OS keyboard commands.
 *
 * A hidden "leader" key (`r`) arms a short window; the next key is a command.
 * The handler ignores real text input (typing in a field) so a command never
 * fires while the user is entering text. The emergency chord works the same way
 * everywhere and goes straight to the standalone `/recovery` system.
 */
const LEADER_TIMEOUT_MS = 1500;
let leaderArmed = false;
let leaderTimer: number | undefined;

function isTextTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return element.tagName === "INPUT" || element.tagName === "TEXTAREA" || element.tagName === "SELECT" || element.isContentEditable;
}

function go(path: string): void {
  window.location.assign(path);
}

/** The help/overview overlay (`r` then `?`). */
const helpListeners = new Set<() => void>();
let helpOpen = false;
function emitHelp() { for (const listener of helpListeners) listener(); }
export function toggleHelp(): void { helpOpen = !helpOpen; emitHelp(); }
export function closeHelp(): void { if (helpOpen) { helpOpen = false; emitHelp(); } }
export function useHelpOpen(): boolean {
  return useSyncExternalStore(
    (listener) => { helpListeners.add(listener); return () => { helpListeners.delete(listener); }; },
    () => helpOpen,
    () => false
  );
}

/** True when this is the emergency secure-attention chord. */
export function isEmergencyChord(event: KeyboardEvent): boolean {
  const ctrlAlt = event.ctrlKey && event.altKey;
  const deleteKey = event.key === "Delete" || event.key === "Backspace";
  if (ctrlAlt && (deleteKey || event.key.toLowerCase() === "e")) return true;
  return event.ctrlKey && event.shiftKey && event.key === "Escape";
}

export function installShortcuts(): () => void {
  const onKey = (event: KeyboardEvent) => {
    // Commands are never triggered while typing into a text field.
    if (isTextTarget(event.target)) return;
    // Emergency always goes to the standalone recovery system.
    if (isEmergencyChord(event)) {
      event.preventDefault();
      go("/recovery");
      return;
    }
    if (leaderArmed) {
      leaderArmed = false;
      clearTimeout(leaderTimer);
      switch (event.key) {
        case "?": event.preventDefault(); toggleHelp(); break;
        case "e": case "E": event.preventDefault(); go("/recovery"); break;
        case "a": case "A": event.preventDefault(); go("/activity"); break;
        case "c": case "C": case "s": case "S": event.preventDefault(); openEmergency(); break;
        case "h": case "H": event.preventDefault(); toggleHelp(); break;
        default: break;
      }
      return;
    }
    if (event.key === "r" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      leaderArmed = true;
      clearTimeout(leaderTimer);
      leaderTimer = window.setTimeout(() => { leaderArmed = false; }, LEADER_TIMEOUT_MS);
    }
  };
  window.addEventListener("keydown", onKey);
  return () => { window.removeEventListener("keydown", onKey); clearTimeout(leaderTimer); };
}
