/**
 * Device-local position of the companion on the desktop/launcher.
 *
 * Stored separately from the pet's appearance and keyed by account owner and
 * pet id, so moving a pet does not mutate its design, accounts on the same
 * browser keep their own spot, and each zoo resident remembers its own place.
 * Coordinates are fractions of the workspace.
 */
const STORAGE_PREFIX = "rumahl.pets.position.v1:";

export interface PetPosition {
  x: number;
  y: number;
}

export const DEFAULT_PET_POSITION: PetPosition = { x: 0.84, y: 0.78 };

/** Keep the companion clear of the top bar and the bottom dock. */
const MIN_X = 0.16;
const MAX_X = 0.84;
const MIN_Y = 0.28;
const MAX_Y = 0.82;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function clampPetPosition(value: PetPosition): PetPosition {
  return {
    x: clamp(value.x, MIN_X, MAX_X),
    y: clamp(value.y, MIN_Y, MAX_Y)
  };
}

function key(owner: string, petId?: string): string {
  return petId ? `${STORAGE_PREFIX}${owner}:${petId}` : `${STORAGE_PREFIX}${owner}`;
}

function valid(value: unknown): value is PetPosition {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.x === "number" &&
    Number.isFinite(candidate.x) &&
    typeof candidate.y === "number" &&
    Number.isFinite(candidate.y)
  );
}

function readPosition(owner: string, petId?: string): PetPosition | null {
  const raw = localStorage.getItem(key(owner, petId));
  if (!raw) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return valid(parsed) ? clampPetPosition(parsed) : null;
  } catch {
    return null;
  }
}

export function getPetPosition(owner: string | null, petId?: string): PetPosition {
  if (!owner || typeof window === "undefined") return DEFAULT_PET_POSITION;
  try {
    // Per-pet spots fall back to the pre-zoo shared spot, then the default.
    if (petId) {
      const own = readPosition(owner, petId);
      if (own) return own;
    }
    return readPosition(owner) ?? DEFAULT_PET_POSITION;
  } catch {
    return DEFAULT_PET_POSITION;
  }
}

export function savePetPosition(owner: string | null, value: PetPosition, petId?: string): PetPosition | null {
  if (!owner) return null;
  const next = clampPetPosition(value);
  try {
    localStorage.setItem(key(owner, petId), JSON.stringify(next));
  } catch {
    return null;
  }
  return next;
}

export function resetPetPosition(owner: string | null, petId?: string): void {
  if (!owner) return;
  try {
    localStorage.removeItem(key(owner, petId));
  } catch {
    // Storage may be unavailable; the default position still applies.
  }
}
