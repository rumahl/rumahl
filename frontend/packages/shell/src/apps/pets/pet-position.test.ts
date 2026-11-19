import { beforeEach, expect, test } from "vitest";
import {
  DEFAULT_PET_POSITION,
  getPetPosition,
  resetPetPosition,
  savePetPosition
} from "./pet-position";

beforeEach(() => localStorage.clear());

test("keeps per-owner positions and falls back to the default", () => {
  expect(getPetPosition("owner-a")).toEqual(DEFAULT_PET_POSITION);
  savePetPosition("owner-a", { x: 0.4, y: 0.5 });
  expect(getPetPosition("owner-a")).toEqual({ x: 0.4, y: 0.5 });
  expect(getPetPosition("owner-b")).toEqual(DEFAULT_PET_POSITION);
  resetPetPosition("owner-a");
  expect(getPetPosition("owner-a")).toEqual(DEFAULT_PET_POSITION);
});

test("clamps positions away from the chrome and ignores invalid data", () => {
  expect(savePetPosition("owner-a", { x: -2, y: 9 })).toEqual({ x: 0.16, y: 0.82 });
  localStorage.setItem("rumahl.pets.position.v1:owner-a", "{oops");
  expect(getPetPosition("owner-a")).toEqual(DEFAULT_PET_POSITION);
  localStorage.setItem("rumahl.pets.position.v1:owner-a", JSON.stringify({ x: "left", y: 1 }));
  expect(getPetPosition("owner-a")).toEqual(DEFAULT_PET_POSITION);
});

test("keeps a separate spot per pet and falls back to the shared one", () => {
  savePetPosition("owner-a", { x: 0.3, y: 0.4 });
  savePetPosition("owner-a", { x: 0.6, y: 0.7 }, "pet-two");
  expect(getPetPosition("owner-a", "pet-two")).toEqual({ x: 0.6, y: 0.7 });
  expect(getPetPosition("owner-a", "pet-unknown")).toEqual({ x: 0.3, y: 0.4 });
  resetPetPosition("owner-a", "pet-two");
  expect(getPetPosition("owner-a", "pet-two")).toEqual({ x: 0.3, y: 0.4 });
});
