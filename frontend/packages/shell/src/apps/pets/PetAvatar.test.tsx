import { render } from "@testing-library/react";
import { expect, test } from "vitest";
import { DEFAULT_PET } from "./pet-model";
import { PetAvatar } from "./PetAvatar";

test("solid pets keep the opaque body", () => {
  const { container } = render(<PetAvatar pet={{ ...DEFAULT_PET, material: "solid" }} />);
  expect(container.querySelector(".pet-avatar")).toHaveClass("pet-material-solid");
  expect(container.querySelector("linearGradient")).toBeNull();
});

test("glass pets get a gradient body and a blurred bloom", () => {
  const { container } = render(<PetAvatar pet={{ ...DEFAULT_PET, material: "glass" }} />);
  expect(container.querySelector(".pet-avatar")).toHaveClass("pet-material-glass");
  expect(container.querySelector("linearGradient")).not.toBeNull();
  expect(container.querySelector("feGaussianBlur")).not.toBeNull();
});

test("plush pets get stitched fabric, a tag and button eyes", () => {
  const { container } = render(<PetAvatar pet={{ ...DEFAULT_PET, material: "plush", eyes: "button" }} />);
  expect(container.querySelector(".pet-avatar")).toHaveClass("pet-material-plush");
  expect(container.querySelector("clipPath")).not.toBeNull();
  expect(container.querySelector("pattern")).not.toBeNull();
  expect(container.querySelector(".pet-avatar__seam")).not.toBeNull();
  expect(container.querySelector(".pet-avatar__tag")).not.toBeNull();
  expect(container.querySelector(".pet-avatar__eyes--button")).not.toBeNull();
});

test("bear and puppy bodies add ears and a muzzle", () => {
  for (const body of ["bear", "puppy"] as const) {
    const { container, unmount } = render(<PetAvatar pet={{ ...DEFAULT_PET, body, material: "plush" }} />);
    expect(container.querySelector(".pet-avatar__body")).not.toBeNull();
    expect(container.querySelector('ellipse[fill="#fff"]')).not.toBeNull();
    unmount();
  }
});

test("renders hairy plush styles", () => {
  for (const hair of ["tuft", "fringe", "curls", "mane", "long", "fluff"] as const) {
    const { container, unmount } = render(<PetAvatar pet={{ ...DEFAULT_PET, hair }} />);
    expect(container.querySelector(".pet-avatar__hair")?.childElementCount).toBeGreaterThan(0);
    unmount();
  }
  const { container } = render(<PetAvatar pet={{ ...DEFAULT_PET, hair: "none" }} />);
  expect(container.querySelector(".pet-avatar__hair")?.childElementCount).toBe(0);
});

test("fur pets get dense surface fur instead of a rim", () => {
  const { container } = render(<PetAvatar pet={{ ...DEFAULT_PET, material: "fur" }} />);
  expect(container.querySelector(".pet-avatar")).toHaveClass("pet-material-fur");
  expect(container.querySelectorAll(".pet-avatar__fur path").length).toBeGreaterThan(80);
  expect(container.querySelector(".pet-avatar__fur")).not.toBeNull();
});

test("renders tails and the new accessories", () => {
  for (const tail of ["stub", "fluffy", "curl"] as const) {
    const { container, unmount } = render(<PetAvatar pet={{ ...DEFAULT_PET, tail }} />);
    expect(container.querySelector(`.pet-avatar__tail--${tail}`)).not.toBeNull();
    unmount();
  }
  const glasses = render(<PetAvatar pet={{ ...DEFAULT_PET, accessory: "glasses" }} />);
  expect(glasses.container.querySelector(".pet-avatar__glasses")).not.toBeNull();
  glasses.unmount();
  const flower = render(<PetAvatar pet={{ ...DEFAULT_PET, accessory: "flower" }} />);
  expect(flower.container.querySelector(".pet-avatar__flower")).not.toBeNull();
});
