import { beforeEach, describe, expect, test } from "vitest";
import { DEFAULT_PET } from "./pet-model";
import {
  applyLifeAction,
  deleteLife,
  effectiveSleeping,
  evolveLife,
  getLife,
  initialLife,
  isRoutineSleep,
  parseLife,
  saveLife,
  type PetLife
} from "./pet-life";

const noon = new Date(2026, 10, 10, 14, 0, 0).getTime();

beforeEach(() => localStorage.clear());

function life(overrides: Partial<PetLife> = {}): PetLife {
  return { ...initialLife(DEFAULT_PET, noon), ...overrides };
}

describe("pet life model", () => {
  test("starts with temperament-dependent energy", () => {
    expect(initialLife({ ...DEFAULT_PET, energy: "calm" }, noon).energy).toBe(80);
    expect(initialLife({ ...DEFAULT_PET, energy: "playful" }, noon).energy).toBe(74);
    expect(initialLife(null, noon).mood).toBe("curious");
  });

  test("evolves needs over real time and keeps a lasting mood", () => {
    const start = life();
    const oneHour = evolveLife(start, DEFAULT_PET, noon + 60 * 60_000);
    expect(oneHour.energy).toBeLessThan(start.energy);
    expect(oneHour.hunger).toBeGreaterThan(start.hunger);
    expect(oneHour.bond).toBeLessThan(start.bond);

    const noRoutine = { ...DEFAULT_PET, sleepStart: 0, sleepEnd: 0 };
    const tired = evolveLife(start, noRoutine, noon + 8 * 60 * 60_000);
    expect(tired.energy).toBeLessThan(30);
    expect(tired.mood).toBe("sleepy");

    const hungry = evolveLife(life({ hunger: 80 }), DEFAULT_PET, noon + 30 * 60_000);
    expect(hungry.mood).toBe("hungry");
  });

  test("sleep restores energy and wakes up when full", () => {
    const asleep = life({ energy: 50, sleeping: true, mood: "sleeping" });
    const rested = evolveLife(asleep, DEFAULT_PET, noon + 60 * 60_000);
    expect(rested.energy).toBeGreaterThan(asleep.energy);
    expect(rested.mood).toBe("sleeping");
    const awake = evolveLife(rested, DEFAULT_PET, noon + 80 * 60 * 60_000);
    expect(awake.energy).toBe(100);
    expect(awake.sleeping).toBe(false);
  });

  test("follows a daily sleep routine across midnight", () => {
    const late = new Date(2026, 10, 10, 23, 0, 0).getTime();
    const morning = new Date(2026, 10, 11, 6, 30, 0).getTime();
    const wake = new Date(2026, 10, 11, 8, 0, 0).getTime();
    expect(isRoutineSleep(DEFAULT_PET, late)).toBe(true);
    expect(isRoutineSleep(DEFAULT_PET, morning)).toBe(true);
    expect(isRoutineSleep(DEFAULT_PET, wake)).toBe(false);
    expect(isRoutineSleep({ ...DEFAULT_PET, sleepStart: 0, sleepEnd: 0 }, late)).toBe(false);

    expect(initialLife(DEFAULT_PET, late).mood).toBe("sleeping");
    const evolved = evolveLife(initialLife(DEFAULT_PET, noon), DEFAULT_PET, late);
    expect(evolved.mood).toBe("sleeping");
    expect(evolved.energy).toBeGreaterThan(70);
    expect(effectiveSleeping(evolved, DEFAULT_PET, late)).toBe(true);
  });

  test("an interaction wakes a routine-sleeping pet for a few minutes", () => {
    const night = new Date(2026, 10, 10, 23, 0, 0).getTime();
    const sleeping = initialLife(DEFAULT_PET, night);
    expect(effectiveSleeping(sleeping, DEFAULT_PET, night)).toBe(true);
    const awake = applyLifeAction(sleeping, "pet", DEFAULT_PET, night + 1000);
    expect(effectiveSleeping(awake, DEFAULT_PET, night + 1000)).toBe(false);
    const later = evolveLife(awake, DEFAULT_PET, night + 6 * 60_000);
    expect(effectiveSleeping(later, DEFAULT_PET, night + 6 * 60_000)).toBe(true);
  });

  test("interactions change needs and wake or put the pet to sleep", () => {
    const fed = applyLifeAction(life({ hunger: 70 }), "feed", DEFAULT_PET, noon + 1000);
    expect(fed.hunger).toBe(46);
    expect(fed.bond).toBeGreaterThan(56);

    const napping = applyLifeAction(fed, "nap", DEFAULT_PET, noon + 2000);
    expect(napping.sleeping).toBe(true);
    expect(napping.energy).toBeGreaterThan(fed.energy);

    const petted = applyLifeAction(napping, "pet", DEFAULT_PET, noon + 3000);
    expect(petted.sleeping).toBe(false);
    expect(petted.bond).toBeGreaterThan(napping.bond);

    const played = applyLifeAction(petted, "play", DEFAULT_PET, noon + 4000);
    expect(played.energy).toBeLessThan(petted.energy);
    expect(["playful", "cozy"]).toContain(played.mood);
  });

  test("rejects malformed stored life", () => {
    expect(parseLife(life())).not.toBeNull();
    expect(parseLife({ ...life(), version: 2 })).toBeNull();
    expect(parseLife({ ...life(), energy: 101 })).toBeNull();
    expect(parseLife({ ...life(), mood: "angry" })).toBeNull();
    expect(parseLife({ ...life(), sleeping: "yes" })).toBeNull();
  });

  test("persists per pet and evolves on read", () => {
    saveLife("owner-a", "pet-one", life({ hunger: 40 }));
    expect(getLife("owner-a", "pet-one", DEFAULT_PET, noon + 60_000)?.hunger).toBeGreaterThan(40);
    expect(getLife("owner-a", "pet-two", DEFAULT_PET, noon)?.hunger).toBe(32);
    deleteLife("owner-a", "pet-one");
    expect(getLife("owner-a", "pet-one", DEFAULT_PET, noon)?.hunger).toBe(32);
  });
});
