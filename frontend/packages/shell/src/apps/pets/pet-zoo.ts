import {
  DEFAULT_PET,
  parsePet,
  randomPet,
  sanitizePetName,
  type PetConfig
} from "./pet-model";

/** A pet keeps its stable identity inside the zoo; appearance stays a PetConfig. */
export interface ZooPet {
  id: string;
  config: PetConfig;
}

/**
 * The account's zoo. `activeId` is the companion currently shown on the
 * desktop/launcher; every other pet lives in the studio collection.
 */
export interface PetZoo {
  version: 2;
  activeId: string;
  pets: readonly ZooPet[];
}

export const MAX_PETS = 8;

const NAMES = ["Momo", "Nova", "Pixel", "Pip", "Mochi", "Luna", "Biscuit", "Ziggy", "Juno", "Taro"];
const PET_ID = /^[a-z0-9][a-z0-9-]{1,63}$/;

export function validPetId(value: unknown): value is string {
  return typeof value === "string" && PET_ID.test(value);
}

export function newPetId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID().toLowerCase();
  }
  return `pet-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function uniqueName(base: string, taken: readonly string[]): string {
  const names = new Set(taken.map((value) => value.toLowerCase()));
  if (!names.has(base.toLowerCase())) return base;
  for (let index = 2; index <= 99; index += 1) {
    const candidate = `${base} ${index}`;
    if (!names.has(candidate.toLowerCase())) return candidate;
  }
  return base;
}

/** Creates a new random pet without touching storage. */
export function createPet(options: { name?: string; config?: Partial<PetConfig>; takenNames?: readonly string[] } = {}): ZooPet {
  const name = sanitizePetName(options.name ?? NAMES[Math.floor(Math.random() * NAMES.length)] ?? "Momo");
  const config: PetConfig = {
    ...DEFAULT_PET,
    ...randomPet(),
    ...options.config,
    name: uniqueName(name || DEFAULT_PET.name, options.takenNames ?? []),
    quiet: options.config?.quiet ?? false
  };
  return { id: newPetId(), config };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Strict parser for the stored zoo. Unknown or duplicate data is rejected. */
export function parseZoo(value: unknown): PetZoo | null {
  if (!isRecord(value) || value.version !== 2) return null;
  if (typeof value.activeId !== "string" || !validPetId(value.activeId)) return null;
  if (!Array.isArray(value.pets) || value.pets.length === 0 || value.pets.length > MAX_PETS) return null;
  const ids = new Set<string>();
  const pets: ZooPet[] = [];
  for (const entry of value.pets) {
    if (!isRecord(entry) || typeof entry.id !== "string" || !validPetId(entry.id)) return null;
    if (ids.has(entry.id)) return null;
    const config = parsePet(entry.config);
    if (!config) return null;
    ids.add(entry.id);
    pets.push({ id: entry.id, config });
  }
  if (!ids.has(value.activeId)) return null;
  return { version: 2, activeId: value.activeId, pets };
}

/** Wraps a pre-zoo single pet so existing accounts keep their companion. */
export function migratePet(pet: PetConfig): PetZoo {
  const id = "legacy-pet";
  return { version: 2, activeId: id, pets: [{ id, config: pet }] };
}

export function activePet(zoo: PetZoo | null): ZooPet | null {
  if (!zoo) return null;
  return zoo.pets.find((pet) => pet.id === zoo.activeId) ?? null;
}

/** Adds a pet, makes it active and keeps the configured palette/name unique. */
export function addZooPet(zoo: PetZoo | null, pet: ZooPet): PetZoo | null {
  if (zoo && zoo.pets.length >= MAX_PETS) return null;
  if (zoo?.pets.some((candidate) => candidate.id === pet.id)) return null;
  const name = uniqueName(
    pet.config.name,
    zoo?.pets.map((candidate) => candidate.config.name) ?? []
  );
  const next = { ...pet, config: { ...pet.config, name } };
  return { version: 2, activeId: next.id, pets: [...(zoo?.pets ?? []), next] };
}

export function updateZooPet(zoo: PetZoo, id: string, config: PetConfig): PetZoo | null {
  if (!zoo.pets.some((pet) => pet.id === id)) return null;
  return {
    ...zoo,
    pets: zoo.pets.map((pet) => (pet.id === id ? { ...pet, config } : pet))
  };
}

export function removeZooPet(zoo: PetZoo, id: string): PetZoo | null {
  const pets = zoo.pets.filter((pet) => pet.id !== id);
  if (pets.length === 0) return null;
  const activeId = zoo.activeId === id ? pets[0]!.id : zoo.activeId;
  return { version: 2, activeId, pets };
}

export function setZooActive(zoo: PetZoo, id: string): PetZoo | null {
  if (!zoo.pets.some((pet) => pet.id === id)) return null;
  return { ...zoo, activeId: id };
}

/** Copies a pet with a fresh id and a numbered name. */
export function duplicateZooPet(zoo: PetZoo, id: string): PetZoo | null {
  const source = zoo.pets.find((pet) => pet.id === id);
  if (!source || zoo.pets.length >= MAX_PETS) return null;
  const copy = createPet({
    name: source.config.name,
    config: source.config,
    takenNames: zoo.pets.map((pet) => pet.config.name)
  });
  return { version: 2, activeId: copy.id, pets: [...zoo.pets, copy] };
}
