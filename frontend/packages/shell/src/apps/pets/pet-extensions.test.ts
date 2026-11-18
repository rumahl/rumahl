import { beforeEach, describe, expect, test, vi } from "vitest";
import {
  MAX_PET_EXTENSIONS,
  dispatchPetCommand,
  getPetPublicState,
  invokePetAction,
  listPetActions,
  publishPetInteraction,
  publishPetState,
  registerPetAction,
  removePetAction,
  resetPetExtensions,
  subscribePetCommands,
  subscribePetInteractions
} from "./pet-extensions";

beforeEach(() => resetPetExtensions());

describe("pet extension registry", () => {
  test("registers a namespaced action and invokes its handler", () => {
    const invoke = vi.fn();
    const unregister = registerPetAction({
      id: "com.example.radio.dance",
      label: "  Radio dance  ",
      icon: "music",
      appId: "com.example.radio",
      invoke
    });
    expect(unregister).not.toBeNull();
    expect(listPetActions()).toMatchObject([{ id: "com.example.radio.dance", label: "Radio dance", icon: "music" }]);
    expect(invokePetAction("com.example.radio.dance")).toBe(true);
    expect(invoke).toHaveBeenCalledTimes(1);
    unregister!();
    expect(listPetActions()).toHaveLength(0);
    expect(invokePetAction("com.example.radio.dance")).toBe(false);
  });

  test("rejects bad ids, labels, icons and foreign replacements", () => {
    expect(registerPetAction({ id: "dance", label: "Dance", invoke: vi.fn() })).toBeNull();
    expect(registerPetAction({ id: "com.example.radio.dance", label: "", invoke: vi.fn() })).toBeNull();
    expect(registerPetAction({ id: "com.example.radio.dance", label: "Dance", icon: "evil" as never, invoke: vi.fn() })).toBeNull();
    expect(registerPetAction({ id: "com.example.radio.dance", label: "Dance", appId: "broker", invoke: vi.fn() })).toBeNull();
    expect(registerPetAction({ id: "com.example.radio.dance", label: "Dance", appId: "com.example.radio", invoke: vi.fn() })).not.toBeNull();
    expect(registerPetAction({ id: "com.example.radio.dance", label: "Other", appId: "com.example.other", invoke: vi.fn() })).toBeNull();
    expect(removePetAction("com.example.radio.dance")).toBe(true);
  });

  test("caps the number of extension actions", () => {
    for (let index = 0; index < MAX_PET_EXTENSIONS; index += 1) {
      expect(registerPetAction({ id: `com.example.app.action${index}`, label: `Action ${index}`, invoke: vi.fn() })).not.toBeNull();
    }
    expect(registerPetAction({ id: "com.example.app.overflow", label: "Overflow", invoke: vi.fn() })).toBeNull();
  });

  test("broadcasts built-in commands and rejects unknown ones", () => {
    const listener = vi.fn();
    const stop = subscribePetCommands(listener);
    expect(dispatchPetCommand("dance", "app:com.example.radio")).toBe(true);
    expect(listener).toHaveBeenCalledWith({ action: "dance", source: "app:com.example.radio" });
    expect(dispatchPetCommand("launch-missiles")).toBe(false);
    stop();
  });

  test("publishes app-safe pet state and interactions", () => {
    const state = { id: "pet-one", name: "Momo", quiet: true, render: "3d" as const, temperament: "calm" as const, mood: "cozy" as const, vitality: 80, hunger: 20, bond: 70, sleeping: false };
    publishPetState(state);
    expect(getPetPublicState()).toEqual(state);
    const listener = vi.fn();
    const stop = subscribePetInteractions(listener);
    publishPetInteraction({ action: "dance", petId: "pet-one", source: "app:com.example.radio" });
    expect(listener).toHaveBeenCalledWith({ action: "dance", petId: "pet-one", source: "app:com.example.radio" });
    stop();
  });
});
