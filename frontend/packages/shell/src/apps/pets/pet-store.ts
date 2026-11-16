import { useSyncExternalStore } from "react";
import {
  parsePet,
  sanitizePetName,
  type PetConfig
} from "./pet-model";
import { deleteLife } from "./pet-life";
import {
  addZooPet,
  activePet,
  duplicateZooPet,
  migratePet,
  newPetId,
  parseZoo,
  removeZooPet,
  setZooActive,
  updateZooPet,
  type PetZoo,
  type ZooPet
} from "./pet-zoo";

/**
 * Device-local pet zoo, namespaced by account owner id.
 *
 * The shell has no generic per-app key/value backend; like themes, window
 * positions and launcher order, pets are browser-local. The owner id from the
 * server preferences keeps accounts on the same browser apart.
 *
 * Version 2 stores a zoo (multiple pets + the active companion). Existing v1
 * single pets are migrated on first read.
 */
const ZOO_PREFIX = "rumahl.pets.zoo.v2:";
const LEGACY_PREFIX = "rumahl.pets.v1:";
const listeners = new Set<() => void>();

let cachedOwner: string | null = null;
let cachedRaw: string | null = null;
let cachedZoo: PetZoo | null = null;
let storageBound = false;

function zooKey(owner: string): string {
  return `${ZOO_PREFIX}${owner}`;
}

function legacyKey(owner: string): string {
  return `${LEGACY_PREFIX}${owner}`;
}

function read(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function parse(raw: string | null): PetZoo | null {
  if (!raw) return null;
  try {
    return parseZoo(JSON.parse(raw));
  } catch {
    return null;
  }
}

function emit(): void {
  for (const listener of listeners) listener();
}

function bindStorage(): void {
  if (storageBound || typeof window === "undefined") return;
  storageBound = true;
  window.addEventListener("storage", (event) => {
    if (!event.key || (!event.key.startsWith(ZOO_PREFIX) && !event.key.startsWith(LEGACY_PREFIX))) return;
    cachedOwner = null;
    cachedRaw = null;
    cachedZoo = null;
    emit();
  });
}

/** Current validated zoo for an owner, or null. Migrates a legacy v1 pet. */
export function getZoo(owner: string | null): PetZoo | null {
  if (!owner) return null;
  const raw = read(zooKey(owner));
  if (cachedOwner === owner && cachedRaw === raw) return cachedZoo;

  let zoo = parse(raw);
  if (!zoo && raw === null) {
    const legacyRaw = read(legacyKey(owner));
    if (legacyRaw) {
      try {
        const legacy = parsePet(JSON.parse(legacyRaw));
        if (legacy) {
          zoo = migratePet(legacy);
          try {
            const migrated = JSON.stringify(zoo);
            localStorage.setItem(zooKey(owner), migrated);
            cachedOwner = owner;
            cachedRaw = migrated;
            cachedZoo = zoo;
            return zoo;
          } catch {
            // Keep the in-memory migration even when storage is unavailable.
          }
        }
      } catch {
        // Corrupted legacy data is treated as no pet.
      }
    }
  }

  cachedOwner = owner;
  cachedRaw = raw;
  cachedZoo = zoo;
  return zoo;
}

/** Stores a validated zoo. Returns the normalized zoo, or null when invalid. */
export function saveZoo(owner: string | null, value: PetZoo): PetZoo | null {
  if (!owner) return null;
  const next = parseZoo(value);
  if (!next) return null;
  const raw = JSON.stringify(next);
  try {
    localStorage.setItem(zooKey(owner), raw);
  } catch {
    return null;
  }
  cachedOwner = owner;
  cachedRaw = raw;
  cachedZoo = next;
  emit();
  return next;
}

/** Adds a pet to the zoo and makes it active. */
export function addPet(owner: string | null, pet: ZooPet): PetZoo | null {
  if (!owner) return null;
  const next = addZooPet(getZoo(owner), pet);
  return next ? saveZoo(owner, next) : null;
}

/** Updates one pet's appearance without changing the active companion. */
export function updatePet(owner: string | null, id: string, config: PetConfig): PetZoo | null {
  if (!owner) return null;
  const zoo = getZoo(owner);
  if (!zoo) return null;
  const next = updateZooPet(zoo, id, config);
  return next ? saveZoo(owner, next) : null;
}

/** Removes one pet. Removing the last pet removes the zoo entirely. */
export function removePet(owner: string | null, id: string): PetZoo | null {
  if (!owner) return null;
  const zoo = getZoo(owner);
  if (!zoo) return null;
  const next = removeZooPet(zoo, id);
  if (!next) {
    deleteZoo(owner);
    return null;
  }
  return saveZoo(owner, next);
}

export function setActivePet(owner: string | null, id: string): PetZoo | null {
  if (!owner) return null;
  const zoo = getZoo(owner);
  if (!zoo) return null;
  const next = setZooActive(zoo, id);
  return next ? saveZoo(owner, next) : null;
}

export function duplicatePet(owner: string | null, id: string): PetZoo | null {
  if (!owner) return null;
  const zoo = getZoo(owner);
  if (!zoo) return null;
  const next = duplicateZooPet(zoo, id);
  return next ? saveZoo(owner, next) : null;
}

/** Removes the whole zoo for one owner. */
export function deleteZoo(owner: string | null): void {
  if (!owner) return;
  const zoo = getZoo(owner);
  if (zoo) {
    for (const pet of zoo.pets) deleteLife(owner, pet.id);
  }
  try {
    localStorage.removeItem(zooKey(owner));
    localStorage.removeItem(legacyKey(owner));
  } catch {
    return;
  }
  cachedOwner = owner;
  cachedRaw = null;
  cachedZoo = null;
  emit();
}

/* -------------------------------------------------------------------------- */
/* Single-pet compatibility                                                   */
/* -------------------------------------------------------------------------- */

/** The active pet's validated config for an owner, or null. */
export function getPet(owner: string | null): PetConfig | null {
  return activePet(getZoo(owner))?.config ?? null;
}

/**
 * Saves the active pet. Without a zoo this creates one; with a zoo it updates
 * the currently active pet and keeps its id.
 */
export function savePet(owner: string | null, value: PetConfig): PetConfig | null {
  if (!owner) return null;
  const next = parsePet({ ...value, name: sanitizePetName(value.name) });
  if (!next) return null;
  const zoo = getZoo(owner);
  if (!zoo) {
    const id = newPetId();
    const created = saveZoo(owner, { version: 2, activeId: id, pets: [{ id, config: next }] });
    return created ? next : null;
  }
  return updatePet(owner, zoo.activeId, next) ? next : null;
}

/** Removes every pet for one owner. */
export function deletePet(owner: string | null): void {
  deleteZoo(owner);
}

function subscribe(listener: () => void): () => void {
  bindStorage();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** React binding for the whole zoo; SSR-safe. */
export function useZoo(owner: string | null): PetZoo | null {
  return useSyncExternalStore(
    subscribe,
    () => getZoo(owner),
    () => null
  );
}

/** React binding for the active pet; SSR-safe. */
export function usePet(owner: string | null): PetConfig | null {
  const zoo = useZoo(owner);
  return activePet(zoo)?.config ?? null;
}
