import { beforeEach, describe, expect, test } from "vitest";
import { DEFAULT_PET, parsePet, sanitizePetName } from "./pet-model";
import { addPet, deletePet, duplicatePet, getPet, getZoo, removePet, savePet, setActivePet, updatePet } from "./pet-store";
import { createPet } from "./pet-zoo";

beforeEach(() => localStorage.clear());

describe("pet model", () => {
  test("sanitizes names and rejects invalid stored pets", () => {
    expect(sanitizePetName("  Mo\u0000mo   the   pet  ")).toBe("Momo the pet");
    expect(parsePet({ ...DEFAULT_PET, version: 2 })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, bodyColor: "url(javascript:alert(1))" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, eyes: "laser" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, name: "   " })).toBeNull();
  });

  test("keeps the palette ids and normalizes the name", () => {
    const pet = parsePet({ ...DEFAULT_PET, name: "  Nova  " });
    expect(pet).toMatchObject({ name: "Nova", bodyColor: "cream", accentColor: "rose", version: 1 });
  });

  test("supports glass and keeps older solid pets valid", () => {
    expect(parsePet({ ...DEFAULT_PET, material: "glass" })?.material).toBe("glass");
    expect(parsePet({ ...DEFAULT_PET, material: undefined })?.material).toBe("solid");
    expect(parsePet({ ...DEFAULT_PET, material: "chrome" })).toBeNull();
  });

  test("adds fur length/colour, tail and 3D depth defaults", () => {
    const legacy = { ...DEFAULT_PET } as Record<string, unknown>;
    delete legacy.furLength;
    delete legacy.furColor;
    delete legacy.tail;
    delete legacy.depth;
    expect(parsePet(legacy)).toMatchObject({
      furLength: "medium", furColor: "body", tail: "none", depth: "normal"
    });
    expect(parsePet({
      ...DEFAULT_PET, material: "fur", furLength: "long", furColor: "accent", tail: "fluffy", depth: "deep"
    })).toMatchObject({ material: "fur", furLength: "long", furColor: "accent", tail: "fluffy", depth: "deep" });
    expect(parsePet({ ...DEFAULT_PET, furLength: "huge" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, furColor: "rainbow" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, tail: "dragon" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, depth: "infinite" })).toBeNull();
  });

  test("adds hair and a daily sleep routine", () => {
    const legacy = { ...DEFAULT_PET } as Record<string, unknown>;
    delete legacy.hair;
    delete legacy.sleepStart;
    delete legacy.sleepEnd;
    expect(parsePet(legacy)).toMatchObject({ hair: "none", sleepStart: 22 * 60, sleepEnd: 7 * 60 });
    expect(parsePet({ ...DEFAULT_PET, hair: "curls", sleepStart: 21 * 60 + 30, sleepEnd: 6 * 60 })).toMatchObject({
      hair: "curls", sleepStart: 21 * 60 + 30, sleepEnd: 6 * 60
    });
    expect(parsePet({ ...DEFAULT_PET, hair: "spikes" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, sleepStart: 24 * 60 })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, sleepEnd: -1 })).toBeNull();
  });

  test("adds 3D, temperament and quiet defaults for older pets", () => {
    const legacy = { ...DEFAULT_PET } as Record<string, unknown>;
    delete legacy.render;
    delete legacy.energy;
    delete legacy.quiet;
    expect(parsePet(legacy)).toMatchObject({ render: "2d", energy: "balanced", quiet: false });
    expect(parsePet({ ...DEFAULT_PET, render: "3d", energy: "calm", quiet: true })).toMatchObject({
      render: "3d", energy: "calm", quiet: true
    });
    expect(parsePet({ ...DEFAULT_PET, render: "hologram" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, energy: "lazy" })).toBeNull();
    expect(parsePet({ ...DEFAULT_PET, quiet: "yes" })).toBeNull();
  });
});

describe("pet store", () => {
  test("persists a pet per owner and keeps accounts apart", () => {
    const saved = savePet("owner-a", { ...DEFAULT_PET, name: "Nova" });
    expect(saved?.name).toBe("Nova");
    expect(getPet("owner-a")?.name).toBe("Nova");
    expect(getPet("owner-b")).toBeNull();
    savePet("owner-b", { ...DEFAULT_PET, name: "Pixel" });
    expect(getPet("owner-a")?.name).toBe("Nova");
    expect(getPet("owner-b")?.name).toBe("Pixel");
  });

  test("refuses invalid pets and ignores corrupted storage", () => {
    expect(savePet("owner-a", { ...DEFAULT_PET, name: "" })).toBeNull();
    expect(getPet("owner-a")).toBeNull();
    localStorage.setItem("rumahl.pets.v1:owner-a", "{not-json");
    expect(getPet("owner-a")).toBeNull();
    localStorage.setItem("rumahl.pets.v1:owner-a", JSON.stringify({ ...DEFAULT_PET, body: "dragon" }));
    expect(getPet("owner-a")).toBeNull();
  });

  test("removes only the owner's pet", () => {
    savePet("owner-a", DEFAULT_PET);
    savePet("owner-b", DEFAULT_PET);
    deletePet("owner-a");
    expect(getPet("owner-a")).toBeNull();
    expect(getPet("owner-b")).not.toBeNull();
  });

  test("migrates a legacy single pet into the zoo", () => {
    localStorage.setItem("rumahl.pets.v1:owner-a", JSON.stringify({ ...DEFAULT_PET, name: "Pixel" }));
    const zoo = getZoo("owner-a");
    expect(zoo?.activeId).toBe("legacy-pet");
    expect(zoo?.pets[0]?.config.name).toBe("Pixel");
    expect(localStorage.getItem("rumahl.pets.zoo.v2:owner-a")).not.toBeNull();
  });

  test("keeps multiple pets and the active companion", () => {
    const first = createPet({ name: "Nova" });
    const one = addPet("owner-a", first)!;
    expect(one.pets).toHaveLength(1);
    const second = createPet({ name: "Pixel", takenNames: ["Nova"] });
    const two = addPet("owner-a", second)!;
    expect(two.pets).toHaveLength(2);
    expect(two.activeId).toBe(second.id);
    expect(getPet("owner-a")?.name).toBe("Pixel");

    expect(setActivePet("owner-a", first.id)?.activeId).toBe(first.id);
    expect(updatePet("owner-a", first.id, { ...first.config, quiet: true })).not.toBeNull();
    expect(getPet("owner-a")?.quiet).toBe(true);
    expect(duplicatePet("owner-a", first.id)?.pets).toHaveLength(3);

    const remaining = removePet("owner-a", first.id)!;
    expect(remaining.pets).toHaveLength(2);
    expect(getPet("owner-a")?.name).not.toBe("Nova");
    removePet("owner-a", remaining.activeId);
    expect(getZoo("owner-a")?.pets.length).toBeGreaterThan(0);
  });
});
