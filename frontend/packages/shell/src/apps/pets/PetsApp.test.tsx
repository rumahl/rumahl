import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({ owner: "demo" as string | null }));

vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));
vi.mock("../../preferences/ShellPreferences", () => ({
  useShellPreferences: () => ({
    preferences: state.owner ? { ownerId: state.owner } : null
  })
}));
vi.mock("../../shell/toasts", () => ({ showToast: vi.fn() }));

import { PetsApp } from "./PetsApp";
import { getPet, getZoo } from "./pet-store";

beforeEach(() => {
  localStorage.clear();
  state.owner = "demo";
  vi.clearAllMocks();
});

test("creates, edits and removes a pet", () => {
  render(<PetsApp />);

  expect(screen.getByRole("heading", { name: "pets.heading" })).toBeInTheDocument();
  expect(screen.getByText("pets.none")).toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "pets.name" }), { target: { value: "Nova" } });
  fireEvent.click(screen.getByRole("button", { name: "pets.create" }));

  expect(getPet("demo")?.name).toBe("Nova");
  expect(screen.getByRole("button", { name: "pets.save" })).toBeInTheDocument();
  expect(screen.getByText("pets.active")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "pets.deletePet" }));
  expect(getPet("demo")).toBeNull();
  expect(screen.getByText("pets.none")).toBeInTheDocument();
});

test("updates the live preview when a swatch is chosen", () => {
  render(<PetsApp />);
  const mint = screen.getAllByRole("button", { name: "pets.color.mint" })[0]!;
  fireEvent.click(mint);
  expect(mint).toHaveAttribute("aria-pressed", "true");
  expect(document.querySelector(".pets-stage")?.className).toContain("pet-body-color-mint");
});

test("cannot save without an account owner", () => {
  state.owner = null;
  render(<PetsApp />);
  expect(screen.getByText("pets.noOwner")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "pets.create" })).toBeDisabled();
});

test("adds a second pet and switches the active companion", () => {
  render(<PetsApp />);
  fireEvent.click(screen.getByRole("button", { name: "pets.create" }));
  expect(getZoo("demo")?.pets).toHaveLength(1);
  const add = screen.getAllByRole("button", { name: "pets.addPet" })[0]!;
  fireEvent.click(add);
  expect(getZoo("demo")?.pets).toHaveLength(2);
  expect(screen.getByText("pets.zooCount")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "pets.duplicatePet" }));
  expect(getZoo("demo")?.pets).toHaveLength(3);
  fireEvent.click(screen.getByRole("button", { name: "pets.deletePet" }));
  expect(getZoo("demo")?.pets).toHaveLength(2);
});

test("turns on quiet idle and shows the 3D studio preview", () => {
  render(<PetsApp />);
  fireEvent.click(screen.getByRole("button", { name: "pets.create" }));
  fireEvent.click(screen.getByRole("switch", { name: "pets.quietMode" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.save" }));
  expect(getPet("demo")?.quiet).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "pets.render" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.render.3d" }));
  expect(document.querySelector(".pets-stage .pet3d")).not.toBeNull();
});

test("edits hair and the daily sleep routine", () => {
  render(<PetsApp />);
  fireEvent.click(screen.getByRole("button", { name: "pets.create" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.hair" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.hair.curls" }));
  fireEvent.change(screen.getByLabelText("pets.sleepStart"), { target: { value: "21:30" } });
  fireEvent.change(screen.getByLabelText("pets.sleepEnd"), { target: { value: "06:15" } });
  fireEvent.click(screen.getByRole("button", { name: "pets.save" }));
  expect(getPet("demo")).toMatchObject({ hair: "curls", sleepStart: 21 * 60 + 30, sleepEnd: 6 * 60 + 15 });
});

test("edits fur length, fur colour, tail and 3D depth", () => {
  render(<PetsApp />);
  fireEvent.click(screen.getByRole("button", { name: "pets.create" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.material" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.material.fur" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.furLength" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.furLength.long" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.furColor" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.furColor.accent" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.tail" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.tail.fluffy" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.render" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.render.3d" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.depth" }));
  fireEvent.click(screen.getByRole("option", { name: "pets.depth.deep" }));
  fireEvent.click(screen.getByRole("button", { name: "pets.save" }));
  expect(getPet("demo")).toMatchObject({
    material: "fur", furLength: "long", furColor: "accent", tail: "fluffy", render: "3d", depth: "deep"
  });
});
