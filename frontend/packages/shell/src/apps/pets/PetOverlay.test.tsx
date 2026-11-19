import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({ mode: "desktop" as "desktop" | "launcher" | "mobile", owner: "demo" }));
const navigate = vi.hoisted(() => vi.fn());

vi.mock("react-router", () => ({ useNavigate: () => navigate }));
vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));
vi.mock("../../preferences/ShellPreferences", () => ({
  useShellPreferences: () => ({ preferences: { ownerId: state.owner } })
}));
vi.mock("../../shell/ShellContext", () => ({ useShell: () => ({ mode: state.mode }) }));

import { PetOverlay } from "./PetOverlay";
import { DEFAULT_PET } from "./pet-model";
import { addPet, deletePet, getZoo, savePet, setActivePet } from "./pet-store";
import { createPet } from "./pet-zoo";
import { initialLife, saveLife } from "./pet-life";

beforeEach(() => {
  localStorage.clear();
  state.mode = "desktop";
  navigate.mockClear();
});

test("shows interactions, opens the studio and respects placement", () => {
  savePet("demo", { ...DEFAULT_PET, name: "Nova", placement: "desktop" });
  const { unmount } = render(<PetOverlay />);
  const pet = screen.getByRole("button", { name: "pets.interact" });
  expect(screen.getByText("Nova")).toBeInTheDocument();

  fireEvent.click(pet);
  expect(document.querySelector(".pet-overlay__name")?.textContent).toMatch(/pets\.reaction\.pet/);
  fireEvent.click(screen.getByRole("button", { name: "pets.action.feed" }));
  expect(document.querySelector(".pet-overlay__name")?.textContent).toMatch(/pets\.reaction\.feed/);
  fireEvent.click(screen.getByRole("button", { name: "pets.action.dance" }));
  expect(document.querySelector(".pet-overlay__name")?.textContent).toMatch(/pets\.reaction\.dance/);
  fireEvent.click(screen.getByRole("button", { name: "pets.action.bath" }));
  expect(document.querySelector(".pet-overlay__name")?.textContent).toMatch(/pets\.reaction\.bath/);
  fireEvent.click(screen.getByRole("button", { name: "pets.action.boop" }));
  expect(document.querySelector(".pet-overlay__companion")?.className).toContain("is-reacting-boop");
  fireEvent.click(screen.getByRole("button", { name: "pets.action.meditate" }));
  expect(document.querySelector(".pet-overlay__companion")?.className).toContain("is-reacting-meditate");

  fireEvent.click(screen.getByRole("button", { name: "pets.action.edit" }));
  expect(navigate).toHaveBeenCalledWith("/app/pets");
  unmount();

  state.mode = "launcher";
  render(<PetOverlay />);
  expect(screen.queryByRole("button", { name: "pets.interact" })).toBeNull();
  deletePet("demo");
});

test("switches quiet mode and renders the 3D companion", () => {
  savePet("demo", { ...DEFAULT_PET, name: "Nova", placement: "desktop", render: "3d" });
  const { unmount } = render(<PetOverlay />);
  expect(document.querySelector(".pet3d")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "pets.quietOn" }));
  expect(document.querySelector(".pet-overlay__companion")?.className).toContain("is-quiet");
  expect(getZoo("demo")?.pets[0]?.config.quiet).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "pets.quietOff" }));
  expect(document.querySelector(".pet-overlay__companion")?.className).not.toContain("is-quiet");
  unmount();
  deletePet("demo");
});

test("goes to sleep during the configured daily routine", () => {
  const now = new Date();
  const minutes = now.getHours() * 60 + now.getMinutes();
  savePet("demo", {
    ...DEFAULT_PET,
    name: "Nova",
    placement: "desktop",
    sleepStart: (minutes + 1439) % 1440,
    sleepEnd: (minutes + 2) % 1440
  });
  const { unmount } = render(<PetOverlay />);
  expect(document.querySelector(".pet-overlay__companion")?.className).toContain("is-life-sleeping");
  expect(document.querySelector(".pet-overlay__sleep")).not.toBeNull();
  expect(screen.getByText("pets.life.mood.sleeping")).toBeInTheDocument();
  unmount();
  deletePet("demo");
});

test("shows a persistent life mood and wakes a sleeping pet", () => {
  savePet("demo", { ...DEFAULT_PET, name: "Nova", placement: "desktop" });
  const activeId = getZoo("demo")!.activeId;
  saveLife("demo", activeId, { ...initialLife(DEFAULT_PET), energy: 18, sleeping: true, mood: "sleeping" });
  const { unmount } = render(<PetOverlay />);
  const companion = document.querySelector(".pet-overlay__companion");
  expect(companion?.className).toContain("is-life-sleeping");
  expect(document.querySelector(".pet-overlay__sleep")).not.toBeNull();
  expect(screen.getByText("pets.life.mood.sleeping")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "pets.interact" }));
  expect(companion?.className).not.toContain("is-life-sleeping");
  unmount();
  deletePet("demo");
});

test("switches the active pet from the zoo strip", () => {
  const first = createPet({ name: "Nova", config: { ...DEFAULT_PET, placement: "desktop" } });
  addPet("demo", first);
  const second = createPet({ name: "Pixel", takenNames: ["Nova"], config: { ...DEFAULT_PET, placement: "desktop" } });
  addPet("demo", second);
  setActivePet("demo", first.id);
  render(<PetOverlay />);
  expect(screen.getByText("Nova")).toBeInTheDocument();
  const switcher = screen.getByRole("group", { name: "pets.zoo" });
  expect(switcher).toBeInTheDocument();
  fireEvent.click(screen.getAllByRole("button", { name: "pets.switchPet" })[1]!);
  expect(getZoo("demo")?.activeId).toBe(second.id);
  expect(screen.getByText("Pixel")).toBeInTheDocument();
  deletePet("demo");
});

test("drags the pet and stores the new position", () => {
  savePet("demo", { ...DEFAULT_PET, placement: "desktop" });
  render(<PetOverlay />);
  const layer = document.querySelector(".pet-overlay") as HTMLElement;
  Object.defineProperty(layer, "getBoundingClientRect", {
    configurable: true,
    value: () => ({ x: 0, y: 0, left: 0, top: 0, right: 1000, bottom: 800, width: 1000, height: 800, toJSON: () => ({}) })
  });
  const pet = screen.getByRole("button", { name: "pets.interact" });
  fireEvent.pointerDown(pet, { pointerId: 1, clientX: 900, clientY: 600, button: 0 });
  fireEvent.pointerMove(pet, { pointerId: 1, clientX: 500, clientY: 300 });
  fireEvent.pointerUp(pet, { pointerId: 1, clientX: 500, clientY: 300 });
  const activeId = getZoo("demo")?.activeId;
  const stored = JSON.parse(localStorage.getItem(`rumahl.pets.position.v1:demo:${activeId}`) ?? "null");
  expect(stored?.x).toBeCloseTo(0.44);
  expect(stored?.y).toBeCloseTo(0.405);
  expect(navigate).not.toHaveBeenCalled();
});

test("is absent without a pet or outside the desktop and launcher", () => {
  render(<PetOverlay />);
  expect(screen.queryByRole("button", { name: "pets.interact" })).toBeNull();
  savePet("demo", DEFAULT_PET);
  state.mode = "mobile";
  render(<PetOverlay />);
  expect(screen.queryByRole("button", { name: "pets.interact" })).toBeNull();
});
