import { createContext, useCallback, useContext, useEffect, useRef, useSyncExternalStore } from "react";

/**
 * Quick actions an app contributes to the OS topbar while its window is
 * focused — the rumahl equivalent of a macOS app menu. First-party apps
 * register from their window; installed (iframe) apps have no bridge for this
 * yet and simply keep the logo-only topbar.
 */
export interface TopbarQuickAction {
  id: string;
  label: string;
  title?: string;
  disabled?: boolean;
  onSelect: () => void;
}

const EMPTY: readonly TopbarQuickAction[] = [];

// One registration store for the whole shell. Kept outside React state so a
// registration never re-renders the registering app's subtree, and so the
// menu bar can subscribe per focused window with useSyncExternalStore.
const listeners = new Set<() => void>();
const windows = new Map<string, readonly TopbarQuickAction[]>();

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Stores (or, with `null`/empty, removes) one window's quick actions. */
export function registerTopbarActions(
  windowId: string,
  actions: readonly TopbarQuickAction[] | null,
): void {
  const next = actions === null || actions.length === 0 ? null : actions;
  const current = windows.get(windowId) ?? null;
  if (next === current) return;
  if (next === null) windows.delete(windowId);
  else windows.set(windowId, next);
  emit();
}

/** The window that owns the calling React subtree, or `null` outside one. */
export const TopbarWindowContext = createContext<string | null>(null);

/** The quick actions of one window, live from the registration store. */
export function useTopbarActions(windowId: string | null | undefined): readonly TopbarQuickAction[] {
  const getSnapshot = useCallback(
    () => windows.get(windowId ?? "") ?? EMPTY,
    [windowId],
  );
  return useSyncExternalStore(subscribe, getSnapshot, () => EMPTY);
}

/**
 * Registers `actions` for the calling window while it stays mounted. Labels,
 * ids and disabled flags drive re-registration; `onSelect` closures are kept
 * current without re-registering on every render.
 */
export function useRegisterTopbarActions(actions: readonly TopbarQuickAction[] | null): void {
  const windowId = useContext(TopbarWindowContext);
  const latest = useRef(actions);
  latest.current = actions;
  const signature = actions
    ? actions
        .map((action) => `${action.id}\u0000${action.label}\u0000${action.disabled ?? false}`)
        .join("\u0001")
    : "";
  const registered = useRef<readonly TopbarQuickAction[] | null>(null);
  const registeredSignature = useRef<string>("");
  if (registeredSignature.current !== signature) {
    registeredSignature.current = signature;
    registered.current = actions
      ? actions.map((action) => ({
          ...action,
          // Always call through to the freshest closure of this action id.
          onSelect: () =>
            latest.current?.find((current) => current.id === action.id)?.onSelect(),
        }))
      : null;
  }
  useEffect(() => {
    if (!windowId) return;
    registerTopbarActions(windowId, registered.current);
    return () => registerTopbarActions(windowId, null);
  }, [windowId, signature]);
}

/** Resets the store between tests. */
export function clearTopbarActionsForTests(): void {
  windows.clear();
  emit();
}

/** Test helper: the currently registered windows. */
export function topbarActionWindowsForTests(): ReadonlyMap<string, readonly TopbarQuickAction[]> {
  return windows;
}
