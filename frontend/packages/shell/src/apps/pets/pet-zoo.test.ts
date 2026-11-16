import { describe, expect, test } from "vitest";
import { DEFAULT_PET } from "./pet-model";
import {
  MAX_PETS,
  addZooPet,
  createPet,
  duplicateZooPet,
  migratePet,
  parseZoo,
  removeZooPet,
  setZooActive,
  updateZooPet,
  validPetId
} from "./pet-zoo";

const fixed = { id: "pet-one", config: { ...DEFAULT_PET, name: "Momo" } };

describe("pet zoo model", () => {
  test("creates unique, valid pet identities", () => {
    const a = createPet({ name: "Nova" });
    const b = createPet({ name: "Nova", takenNames: ["Nova"] });
    expect(validPetId(a.id)).toBe(true);
    expect(a.config.name).toBe("Nova");
    expect(b.config.name).toBe("Nova 2");
  });

  test("migrates a legacy single pet", () => {
    const zoo = migratePet({ ...DEFAULT_PET, name: "Pixel" });
    expect(zoo.activeId).toBe("legacy-pet");
    expect(parseZoo(zoo)?.pets[0]?.config.name).toBe("Pixel");
  });

  test("parses only strict, unique, non-empty zoos", () => {
    const zoo = { version: 2, activeId: "pet-one", pets: [fixed] };
    expect(parseZoo(zoo)?.pets).toHaveLength(1);
    for (const value of [
      { ...zoo, version: 3 },
      { ...zoo, activeId: "missing" },
      { ...zoo, pets: [] },
      { ...zoo, pets: [fixed, fixed] },
      { ...zoo, pets: [{ id: "Bad Id", config: DEFAULT_PET }] },
      { ...zoo, pets: [{ id: "pet-one", config: { ...DEFAULT_PET, quiet: "yes" } }] }
    ]) expect(parseZoo(value)).toBeNull();
  });

  test("adds, updates, activates and removes pets", () => {
    const added = addZooPet(null, fixed)!;
    expect(added.activeId).toBe("pet-one");
    const second = addZooPet(added, { id: "pet-two", config: { ...DEFAULT_PET, name: "Momo" } })!;
    expect(second.pets.map((pet) => pet.config.name)).toEqual(["Momo", "Momo 2"]);
    expect(second.activeId).toBe("pet-two");

    const updated = updateZooPet(second, "pet-one", { ...DEFAULT_PET, name: "Luna" })!;
    expect(updated.pets[0]?.config.name).toBe("Luna");
    const activated = setZooActive(updated, "pet-one")!;
    expect(activated.activeId).toBe("pet-one");
    const removed = removeZooPet(activated, "pet-one")!;
    expect(removed.activeId).toBe("pet-two");
    expect(removeZooPet({ version: 2, activeId: "pet-one", pets: [fixed] }, "pet-one")).toBeNull();
  });

  test("caps the zoo and duplicates with a fresh identity", () => {
    let zoo = addZooPet(null, fixed)!;
    for (let index = 2; index <= MAX_PETS; index += 1) {
      zoo = addZooPet(zoo, { id: `pet-${index}`, config: { ...DEFAULT_PET, name: `Pet ${index}` } })!;
    }
    expect(zoo.pets).toHaveLength(MAX_PETS);
    expect(addZooPet(zoo, { id: "pet-extra", config: DEFAULT_PET })).toBeNull();
    expect(duplicateZooPet(zoo, "pet-one")).toBeNull();

    const copy = duplicateZooPet(addZooPet(null, fixed)!, "pet-one")!;
    expect(copy.pets).toHaveLength(2);
    expect(copy.pets[1]?.id).not.toBe("pet-one");
    expect(copy.pets[1]?.config.name).toBe("Momo 2");
  });
});
