/**
 * Pet data model.
 *
 * The pet is a small, fully data-driven companion rendered by the shell. Colors
 * are palette ids instead of free-form CSS so stored data can never inject
 * styles or markup.
 */
export const PET_BODIES = ["blob", "dot", "cat", "bunny", "bear", "puppy"] as const;
export type PetBody = (typeof PET_BODIES)[number];

export const PET_EYES = ["dot", "happy", "sleepy", "sparkle", "button"] as const;
export type PetEyes = (typeof PET_EYES)[number];

export const PET_ACCESSORIES = ["none", "leaf", "star", "halo", "headphones", "bow", "scarf", "glasses", "flower"] as const;
export type PetAccessory = (typeof PET_ACCESSORIES)[number];

/** Optional fur patterns on the head. */
export const PET_HAIRS = ["none", "tuft", "fringe", "curls", "mane", "long", "fluff"] as const;
export type PetHair = (typeof PET_HAIRS)[number];

/** Surface fur finish controls for the `fur` material. */
export const PET_FUR_LENGTHS = ["short", "medium", "long"] as const;
export type PetFurLength = (typeof PET_FUR_LENGTHS)[number];

export const PET_FUR_COLORS = ["body", "cream", "accent"] as const;
export type PetFurColor = (typeof PET_FUR_COLORS)[number];

/** Tails sit behind the body and make the silhouette more animal-like. */
export const PET_TAILS = ["none", "stub", "fluffy", "curl"] as const;
export type PetTail = (typeof PET_TAILS)[number];

/** Extrusion depth of the 3D renderer. */
export const PET_3D_DEPTHS = ["soft", "normal", "deep"] as const;
export type Pet3DDepth = (typeof PET_3D_DEPTHS)[number];

export const PET_MOTIONS = ["float", "bounce", "wiggle", "still"] as const;
export type PetMotion = (typeof PET_MOTIONS)[number];

export const PET_PLACEMENTS = ["desktop", "launcher", "both"] as const;
export type PetPlacement = (typeof PET_PLACEMENTS)[number];

export const PET_SIZES = ["sm", "md", "lg"] as const;
export type PetSize = (typeof PET_SIZES)[number];

/** Surface finish: opaque paint or transparent frosted glass. */
export const PET_MATERIALS = ["solid", "glass", "plush", "fur"] as const;
export type PetMaterial = (typeof PET_MATERIALS)[number];

/** Rendering style: the flat SVG companion or a layered, tiltable 3D scene. */
export const PET_RENDERS = ["2d", "3d"] as const;
export type PetRender = (typeof PET_RENDERS)[number];

/** Idle temperament; calm pets move and speak less often. */
export const PET_ENERGIES = ["calm", "balanced", "playful"] as const;
export type PetEnergy = (typeof PET_ENERGIES)[number];

/** Light, friendly pet bodies. */
export const PET_BODY_COLORS = [
  "sky",
  "mint",
  "rose",
  "lilac",
  "amber",
  "coral",
  "snow",
  "sand",
  "cream",
  "cocoa",
  "sage"
] as const;
export type PetBodyColor = (typeof PET_BODY_COLORS)[number];

/** Cheeks, accessories and highlights. */
export const PET_ACCENT_COLORS = [
  "rose",
  "mint",
  "sky",
  "lilac",
  "amber",
  "coral",
  "slate",
  "white"
] as const;
export type PetAccentColor = (typeof PET_ACCENT_COLORS)[number];

export interface PetConfig {
  version: 1;
  name: string;
  body: PetBody;
  bodyColor: PetBodyColor;
  accentColor: PetAccentColor;
  eyes: PetEyes;
  accessory: PetAccessory;
  hair: PetHair;
  furLength: PetFurLength;
  furColor: PetFurColor;
  tail: PetTail;
  motion: PetMotion;
  placement: PetPlacement;
  size: PetSize;
  material: PetMaterial;
  render: PetRender;
  depth: Pet3DDepth;
  energy: PetEnergy;
  /** Quiet pets keep idle time still: no idle chatter, fewer particles. */
  quiet: boolean;
  /** Daily routine in local minutes from midnight; the pet sleeps in this window. */
  sleepStart: number;
  sleepEnd: number;
}

export const DEFAULT_PET: PetConfig = {
  version: 1,
  name: "Momo",
  body: "bear",
  bodyColor: "cream",
  accentColor: "rose",
  eyes: "button",
  accessory: "bow",
  hair: "tuft",
  furLength: "medium",
  furColor: "body",
  tail: "none",
  motion: "float",
  placement: "both",
  size: "md",
  material: "plush",
  render: "2d",
  depth: "normal",
  energy: "balanced",
  quiet: false,
  sleepStart: 22 * 60,
  sleepEnd: 7 * 60
};

const MAX_NAME = 24;

/** Removes control characters, collapses whitespace and caps the name. */
export function sanitizePetName(value: string): string {
  return Array.from(value)
    .filter((character) => {
      const code = character.codePointAt(0) ?? 0;
      return code >= 32 && code !== 127;
    })
    .join("")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_NAME);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validClock(value: unknown, fallback: number): number | null {
  if (value === undefined) return fallback;
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value < 24 * 60
    ? value
    : null;
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : null;
}

/** Strict parser for anything read from storage. Unknown data is rejected. */
export function parsePet(value: unknown): PetConfig | null {
  if (!isRecord(value) || value.version !== 1) return null;
  const name = typeof value.name === "string" ? sanitizePetName(value.name) : "";
  if (!name) return null;
  const body = oneOf(value.body, PET_BODIES);
  const bodyColor = oneOf(value.bodyColor, PET_BODY_COLORS);
  const accentColor = oneOf(value.accentColor, PET_ACCENT_COLORS);
  const eyes = oneOf(value.eyes, PET_EYES);
  const accessory = oneOf(value.accessory, PET_ACCESSORIES);
  const hair = value.hair === undefined ? "none" : oneOf(value.hair, PET_HAIRS);
  const furLength = value.furLength === undefined ? "medium" : oneOf(value.furLength, PET_FUR_LENGTHS);
  const furColor = value.furColor === undefined ? "body" : oneOf(value.furColor, PET_FUR_COLORS);
  const tail = value.tail === undefined ? "none" : oneOf(value.tail, PET_TAILS);
  const motion = oneOf(value.motion, PET_MOTIONS);
  const placement = oneOf(value.placement, PET_PLACEMENTS);
  const size = oneOf(value.size, PET_SIZES);
  // Older pets have no material/render/energy/quiet; keep them valid.
  const material = value.material === undefined ? "solid" : oneOf(value.material, PET_MATERIALS);
  const render = value.render === undefined ? "2d" : oneOf(value.render, PET_RENDERS);
  const depth = value.depth === undefined ? "normal" : oneOf(value.depth, PET_3D_DEPTHS);
  const energy = value.energy === undefined ? "balanced" : oneOf(value.energy, PET_ENERGIES);
  const quiet = value.quiet === undefined ? false : typeof value.quiet === "boolean" ? value.quiet : null;
  const sleepStart = validClock(value.sleepStart, 22 * 60);
  const sleepEnd = validClock(value.sleepEnd, 7 * 60);
  if (!body || !bodyColor || !accentColor || !eyes || !accessory || !hair || !furLength || !furColor || !tail || !motion || !placement || !size || !material || !render || !depth || !energy || quiet === null || sleepStart === null || sleepEnd === null) {
    return null;
  }
  return {
    version: 1, name, body, bodyColor, accentColor, eyes, accessory, hair, furLength, furColor, tail,
    motion, placement, size, material, render, depth, energy, quiet, sleepStart, sleepEnd
  };
}

function pick<T>(values: readonly T[]): T {
  return values[Math.floor(Math.random() * values.length)]!;
}

/** New random look; the name and placement are kept by the caller. */
export function randomPet(): Pick<
  PetConfig,
  "body" | "bodyColor" | "accentColor" | "eyes" | "accessory" | "hair" | "furLength" | "furColor" | "tail" | "motion" | "material" | "render" | "depth" | "energy"
> {
  return {
    body: pick(["bear", "puppy", "bunny", "cat", "blob", "dot", "bear", "puppy"] as const),
    bodyColor: pick(PET_BODY_COLORS),
    accentColor: pick(PET_ACCENT_COLORS),
    eyes: pick(["button", "button", "dot", "happy", "sleepy", "sparkle"] as const),
    accessory: pick(["bow", "scarf", "leaf", "star", "halo", "headphones", "glasses", "flower", "none"] as const),
    hair: pick(["tuft", "fringe", "curls", "mane", "long", "fluff", "none", "tuft", "fluff"] as const),
    furLength: pick(PET_FUR_LENGTHS),
    furColor: pick(PET_FUR_COLORS),
    tail: pick(["none", "stub", "fluffy", "curl", "fluffy", "curl"] as const),
    motion: pick(PET_MOTIONS),
    material: pick(["plush", "plush", "fur", "fur", "fur", "glass", "solid"] as const),
    render: pick(PET_RENDERS),
    depth: pick(PET_3D_DEPTHS),
    energy: pick(PET_ENERGIES)
  };
}
