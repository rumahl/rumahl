import { useSyncExternalStore } from "react";
import {
  isBuiltinPetAction,
  isPetExtensionIcon,
  sanitizePetActionLabel,
  validPetActionId,
  type PetBuiltinAction,
  type PetExtensionIcon
} from "./pet-actions";

/**
 * Open pet interface for other shell programs.
 *
 * A program may register a namespaced action with its own handler. The handler
 * runs in the shell realm, so this API is intentionally not reachable from an
 * installed app's iframe; installed apps use the capability-gated bridge, which
 * registers metadata and receives a `pets.action.triggered` event instead.
 */
export interface PetActionExtension {
  /** Namespaced id, e.g. `com.example.radio.dance`. */
  id: string;
  /** Short label shown in the pet menu. */
  label: string;
  icon?: PetExtensionIcon;
  /** Optional owner app id for diagnostics. */
  appId?: string;
  invoke: () => void | Promise<void>;
}

export interface PetPublicState {
  id: string;
  name: string;
  quiet: boolean;
  render: "2d" | "3d";
  temperament: "calm" | "balanced" | "playful";
  mood: "content" | "curious" | "cozy" | "playful" | "sleepy" | "hungry" | "lonely" | "sleeping";
  vitality: number;
  hunger: number;
  bond: number;
  sleeping: boolean;
}

export interface PetCommand {
  action: PetBuiltinAction;
  source?: string;
}

export interface PetInteraction {
  action: string;
  petId: string;
  source?: string;
}

export const MAX_PET_EXTENSIONS = 12;

const EMPTY_ACTIONS: readonly PetActionExtension[] = [];
const extensionListeners = new Set<() => void>();
const commandListeners = new Set<(command: PetCommand) => void>();
const interactionListeners = new Set<(interaction: PetInteraction) => void>();
const stateListeners = new Set<() => void>();

let actions: readonly PetActionExtension[] = EMPTY_ACTIONS;
let publicState: PetPublicState | null = null;

function emitActions(): void {
  for (const listener of extensionListeners) listener();
}

function validAppId(value: unknown): boolean {
  return typeof value === "string" && value.length <= 255 && /^[a-z0-9]+(?:[._-][a-z0-9]+)+$/.test(value);
}

/** Registers an action. Returns an unregister function, or null when rejected. */
export function registerPetAction(
  input: Omit<PetActionExtension, "invoke"> & { invoke: PetActionExtension["invoke"] }
): (() => void) | null {
  const id = typeof input.id === "string" ? input.id : "";
  const label = sanitizePetActionLabel(input.label);
  if (!validPetActionId(id) || !label || typeof input.invoke !== "function") return null;
  if (input.icon !== undefined && !isPetExtensionIcon(input.icon)) return null;
  if (input.appId !== undefined && !validAppId(input.appId)) return null;

  const existing = actions.find((action) => action.id === id);
  if (existing && existing.appId !== input.appId) return null;
  if (!existing && actions.length >= MAX_PET_EXTENSIONS) return null;

  const next: PetActionExtension = {
    id,
    label,
    ...(input.icon ? { icon: input.icon } : {}),
    ...(input.appId ? { appId: input.appId } : {}),
    invoke: input.invoke
  };
  actions = existing
    ? actions.map((action) => (action.id === id ? next : action))
    : [...actions, next];
  emitActions();

  return () => {
    if (!actions.some((action) => action === next)) return;
    actions = actions.filter((action) => action !== next);
    emitActions();
  };
}

/** Removes an extension action by id. */
export function removePetAction(id: string): boolean {
  const before = actions.length;
  actions = actions.filter((action) => action.id !== id);
  if (actions.length === before) return false;
  emitActions();
  return true;
}

export function listPetActions(): readonly PetActionExtension[] {
  return actions;
}

/** Invokes an extension action. Unknown ids and thrown handlers fail safely. */
export function invokePetAction(id: string): boolean {
  const action = actions.find((candidate) => candidate.id === id);
  if (!action) return false;
  try {
    void Promise.resolve(action.invoke()).catch(() => undefined);
  } catch {
    // A misbehaving extension must not break the pet menu.
  }
  return true;
}

export function subscribePetActions(listener: () => void): () => void {
  extensionListeners.add(listener);
  return () => {
    extensionListeners.delete(listener);
  };
}

export function usePetActions(): readonly PetActionExtension[] {
  return useSyncExternalStore(
    subscribePetActions,
    () => actions,
    () => EMPTY_ACTIONS
  );
}

/** Sends a built-in interaction to every mounted pet overlay. */
export function dispatchPetCommand(action: string, source?: string): boolean {
  if (!isBuiltinPetAction(action)) return false;
  const command: PetCommand = { action, ...(source ? { source } : {}) };
  for (const listener of commandListeners) listener(command);
  return true;
}

export function subscribePetCommands(listener: (command: PetCommand) => void): () => void {
  commandListeners.add(listener);
  return () => {
    commandListeners.delete(listener);
  };
}

/** Publishes an interaction so apps can add their own reactions to it. */
export function publishPetInteraction(interaction: PetInteraction): void {
  for (const listener of interactionListeners) listener(interaction);
}

export function subscribePetInteractions(listener: (interaction: PetInteraction) => void): () => void {
  interactionListeners.add(listener);
  return () => {
    interactionListeners.delete(listener);
  };
}

/** The active pet's public, app-safe state; never includes stored appearance data. */
export function publishPetState(state: PetPublicState | null): void {
  publicState = state;
  for (const listener of stateListeners) listener();
}

export function getPetPublicState(): PetPublicState | null {
  return publicState;
}

export function subscribePetState(listener: () => void): () => void {
  stateListeners.add(listener);
  return () => {
    stateListeners.delete(listener);
  };
}

/** Testing helper: clears registrations and state without touching storage. */
export function resetPetExtensions(): void {
  actions = EMPTY_ACTIONS;
  publicState = null;
  emitActions();
  for (const listener of stateListeners) listener();
}
