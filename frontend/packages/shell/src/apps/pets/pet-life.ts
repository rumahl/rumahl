import { useCallback, useEffect, useState } from "react";
import type { PetConfig } from "./pet-model";

/**
 * Persistent pet needs.
 *
 * The companion is not a short-lived animation: energy, hunger and bond move
 * over real time, survive reloads and drive a lasting mood. A 2.4s reaction is
 * only the visible response to an interaction; the underlying life keeps going.
 */
export const PET_MOODS = [
  "content",
  "curious",
  "cozy",
  "playful",
  "sleepy",
  "hungry",
  "lonely",
  "sleeping"
] as const;
export type PetMood = (typeof PET_MOODS)[number];

export interface PetLife {
  version: 1;
  energy: number;
  hunger: number;
  bond: number;
  mood: PetMood;
  sleeping: boolean;
  /** A recent interaction keeps a normally sleeping pet awake until this time. */
  wakeUntil: number;
  lastInteraction: number;
  updatedAt: number;
}

export type PetLifeAction =
  | "pet"
  | "play"
  | "feed"
  | "nap"
  | "dance"
  | "talk"
  | "bath"
  | "boop"
  | "spin"
  | "stretch"
  | "sing"
  | "peek"
  | "meditate"
  | "custom";

const STORAGE_PREFIX = "rumahl.pets.life.v1:";
const MAX_STAT = 100;

function storageKey(owner: string, petId: string): string {
  return `${STORAGE_PREFIX}${owner}:${petId}`;
}

function clamp(value: number): number {
  return Math.min(MAX_STAT, Math.max(0, Math.round(value * 10) / 10));
}

function validStat(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= MAX_STAT;
}

function validMood(value: unknown): value is PetMood {
  return typeof value === "string" && (PET_MOODS as readonly string[]).includes(value);
}

export function parseLife(value: unknown): PetLife | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  if (record.version !== 1) return null;
  if (!validStat(record.energy) || !validStat(record.hunger) || !validStat(record.bond)) return null;
  if (!validMood(record.mood) || typeof record.sleeping !== "boolean") return null;
  if (typeof record.lastInteraction !== "number" || !Number.isFinite(record.lastInteraction)) return null;
  if (typeof record.updatedAt !== "number" || !Number.isFinite(record.updatedAt)) return null;
  if (record.wakeUntil !== undefined && (typeof record.wakeUntil !== "number" || !Number.isFinite(record.wakeUntil))) return null;
  return {
    version: 1,
    energy: record.energy,
    hunger: record.hunger,
    bond: record.bond,
    mood: record.mood,
    sleeping: record.sleeping,
    wakeUntil: record.wakeUntil ?? 0,
    lastInteraction: record.lastInteraction,
    updatedAt: record.updatedAt
  };
}

export function initialLife(pet: PetConfig | null, now = Date.now()): PetLife {
  const energy = pet?.energy === "calm" ? 80 : pet?.energy === "playful" ? 74 : 70;
  return withMood({
    version: 1,
    energy,
    hunger: 32,
    bond: 56,
    sleeping: false,
    wakeUntil: 0,
    lastInteraction: now - 10 * 60_000,
    updatedAt: now
  }, pet, now);
}

/** True while the pet's configured local bedtime window is active. */
export function isRoutineSleep(pet: PetConfig | null, now = Date.now()): boolean {
  if (!pet || pet.sleepStart === pet.sleepEnd) return false;
  const date = new Date(now);
  const minutes = date.getHours() * 60 + date.getMinutes();
  return pet.sleepStart < pet.sleepEnd
    ? minutes >= pet.sleepStart && minutes < pet.sleepEnd
    : minutes >= pet.sleepStart || minutes < pet.sleepEnd;
}

/** Manual naps and the daily routine both count as sleeping. */
export function effectiveSleeping(life: PetLife | null, pet: PetConfig | null, now = Date.now()): boolean {
  return Boolean(life?.sleeping || (isRoutineSleep(pet, now) && now >= (life?.wakeUntil ?? 0)));
}

function deriveMood(
  energy: number,
  hunger: number,
  bond: number,
  sleeping: boolean,
  lastInteraction: number,
  now: number
): PetMood {
  if (sleeping) return "sleeping";
  if (energy < 24) return "sleepy";
  if (hunger > 82) return "hungry";
  if (bond < 28) return "lonely";
  const quietFor = now - lastInteraction;
  if (quietFor < 8 * 60_000 && bond > 68) return "cozy";
  if (quietFor < 6 * 60_000) return "playful";
  if (energy < 46) return "sleepy";
  if (hunger > 62) return "hungry";
  if (bond < 38) return "lonely";
  const hour = new Date(now).getHours();
  if ((hour >= 23 || hour < 6) && energy < 70) return "sleepy";
  return bond > 62 ? "content" : "curious";
}

function withMood(life: Omit<PetLife, "mood">, pet: PetConfig | null, now: number): PetLife {
  const sleeping = life.sleeping || (isRoutineSleep(pet, now) && now >= life.wakeUntil);
  return {
    ...life,
    mood: deriveMood(life.energy, life.hunger, life.bond, sleeping, life.lastInteraction, now)
  };
}

/** Advances needs by elapsed real time. Sleeping restores energy. */
export function evolveLife(life: PetLife, pet: PetConfig | null, now = Date.now()): PetLife {
  const elapsed = Math.max(0, Math.min(24 * 60, (now - life.updatedAt) / 60_000));
  const calm = pet?.energy === "calm" ? 0.72 : pet?.energy === "playful" ? 1.15 : 1;
  const routine = isRoutineSleep(pet, now) && now >= life.wakeUntil;
  const sleeping = life.sleeping || routine;
  const energy = clamp(life.energy + (sleeping ? elapsed * 0.7 : -elapsed * 0.1 * calm));
  const hunger = clamp(life.hunger + elapsed * 0.075 * calm);
  let bond = life.bond - elapsed * 0.012;
  if (hunger > 90) bond -= elapsed * 0.03;
  const awake = life.sleeping && energy >= 99 ? false : life.sleeping;
  const base = {
    version: 1 as const,
    energy,
    hunger,
    bond: clamp(bond),
    sleeping: awake,
    wakeUntil: life.wakeUntil,
    lastInteraction: life.lastInteraction,
    updatedAt: now
  };
  return withMood(base, pet, now);
}

const EFFECTS: Record<PetLifeAction, { energy: number; hunger: number; bond: number; sleep?: boolean }> = {
  pet: { energy: -0.5, hunger: 0.4, bond: 4 },
  play: { energy: -8, hunger: 3, bond: 6 },
  feed: { energy: 3, hunger: -24, bond: 3 },
  nap: { energy: 26, hunger: 1.5, bond: 1, sleep: true },
  dance: { energy: -6, hunger: 1.5, bond: 5 },
  talk: { energy: -0.5, hunger: 0.2, bond: 2 },
  bath: { energy: -2, hunger: 0.5, bond: 3 },
  boop: { energy: -0.5, hunger: 0.3, bond: 4 },
  spin: { energy: -3, hunger: 1, bond: 3 },
  stretch: { energy: 2, hunger: 0.6, bond: 1 },
  sing: { energy: -2, hunger: 0.8, bond: 4 },
  peek: { energy: 0, hunger: 0.2, bond: 2 },
  meditate: { energy: 8, hunger: 0.5, bond: 2 },
  custom: { energy: -0.5, hunger: 0.3, bond: 2 }
};

export function applyLifeAction(
  life: PetLife,
  action: PetLifeAction,
  pet: PetConfig | null,
  now = Date.now()
): PetLife {
  const effect = EFFECTS[action];
  const base = {
    version: 1 as const,
    energy: clamp(life.energy + effect.energy),
    hunger: clamp(life.hunger + effect.hunger),
    bond: clamp(life.bond + effect.bond),
    sleeping: effect.sleep === true,
    wakeUntil: effect.sleep === true ? 0 : now + 5 * 60_000,
    lastInteraction: now,
    updatedAt: now
  };
  return withMood(base, pet, now);
}

function read(owner: string, petId: string): PetLife | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = localStorage.getItem(storageKey(owner, petId));
    if (!raw) return null;
    return parseLife(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function getLife(owner: string | null, petId: string | null, pet: PetConfig | null, now = Date.now()): PetLife | null {
  if (!owner || !petId) return null;
  const stored = read(owner, petId);
  if (!stored) {
    const initial = initialLife(pet, now);
    saveLife(owner, petId, initial);
    return initial;
  }
  const evolved = evolveLife(stored, pet, now);
  saveLife(owner, petId, evolved);
  return evolved;
}

export function saveLife(owner: string | null, petId: string | null, life: PetLife): PetLife | null {
  if (!owner || !petId) return null;
  const parsed = parseLife(life);
  if (!parsed) return null;
  try {
    localStorage.setItem(storageKey(owner, petId), JSON.stringify(parsed));
  } catch {
    return null;
  }
  return parsed;
}

export function deleteLife(owner: string | null, petId: string | null): void {
  if (!owner || !petId) return;
  try {
    localStorage.removeItem(storageKey(owner, petId));
  } catch {
    // Ignore unavailable storage.
  }
}

/**
 * React binding: keeps the needs moving while the shell is open and persists
 * every interaction. The interval is intentionally longer than the visible
 * reactions so the long-term mood, not a short animation, is what the pet
 * returns to.
 */
export function usePetLife(owner: string | null, petId: string | null, pet: PetConfig | null) {
  const [life, setLife] = useState<PetLife | null>(() => getLife(owner, petId, pet));
  const petEnergy = pet?.energy;
  const petQuiet = pet?.quiet;
  const petSleepStart = pet?.sleepStart;
  const petSleepEnd = pet?.sleepEnd;

  useEffect(() => {
    if (!owner || !petId) {
      setLife(null);
      return;
    }
    const refresh = () => {
      const next = getLife(owner, petId, pet);
      if (next) setLife(next);
    };
    refresh();
    const timer = window.setInterval(refresh, 20_000);
    return () => window.clearInterval(timer);
  }, [owner, petId, petEnergy, petQuiet, petSleepStart, petSleepEnd]);

  const interact = useCallback((action: PetLifeAction) => {
    if (!owner || !petId) return;
    setLife((previous) => {
      const base = previous ?? initialLife(pet);
      const next = applyLifeAction(base, action, pet);
      saveLife(owner, petId, next);
      return next;
    });
  }, [owner, petId, pet]);

  return { life, interact };
}
