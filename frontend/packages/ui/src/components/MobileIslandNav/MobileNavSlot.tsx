import { createContext, useContext, useEffect, useRef } from "react";
import type { MobileIslandNavItem } from "./MobileIslandNav";

/** The navigation an app contributes to the shell's mobile bottom bar. */
export interface MobileNavModel {
  items: readonly MobileIslandNavItem[];
  value: string;
  onChange: (id: string) => void;
  label: string;
}

export interface MobileNavSlot {
  /** Window that owns the current React subtree. */
  windowId: string;
  /** Null removes this window's contribution when the app unmounts. */
  register: (windowId: string, model: MobileNavModel | null) => void;
}

/**
 * Provided by the shell around each app window. Apps register their mobile
 * destinations here; the shell renders one bottom bar with those destinations
 * plus the system Home / Open windows actions.
 *
 * Without a provider (Storybook, isolated UI tests) apps keep rendering their
 * local MobileIslandNav fallback.
 */
export const MobileNavSlotContext = createContext<MobileNavSlot | null>(null);

export function useRegisterMobileNav(model: MobileNavModel | null): boolean {
  const slot = useContext(MobileNavSlotContext);
  const modelRef = useRef(model);
  modelRef.current = model;
  const onChangeRef = useRef(model?.onChange);
  onChangeRef.current = model?.onChange;
  const signature = model
    ? `${model.label}\u0000${model.value}\u0000${model.items.map((item) => `${item.id}\u0001${item.label}`).join("\u0002")}`
    : "";
  const registered = useRef<MobileNavModel | null>(null);
  const registeredSignature = useRef<string | null>(null);

  if (registeredSignature.current !== signature) {
    registeredSignature.current = signature;
    registered.current = model
      ? {
          items: model.items,
          value: model.value,
          label: model.label,
          // Keep the bar's callbacks current without re-registering on every
          // render of an app that recreates its onChange closure.
          onChange: (id) => onChangeRef.current?.(id)
        }
      : null;
  }

  useEffect(() => {
    if (!slot) return;
    slot.register(slot.windowId, registered.current);
    return () => slot.register(slot.windowId, null);
  }, [slot, signature]);

  return Boolean(slot);
}
