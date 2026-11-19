/** Built-in interactions the shell understands without any app code. */
export const PET_ACTION_IDS = [
  "pet",
  "play",
  "feed",
  "nap",
  "dance",
  "talk",
  "bath",
  "boop",
  "spin",
  "stretch",
  "sing",
  "peek",
  "meditate"
] as const;
export type PetBuiltinAction = (typeof PET_ACTION_IDS)[number];

const BUILTIN = new Set<string>(PET_ACTION_IDS);

export function isBuiltinPetAction(value: unknown): value is PetBuiltinAction {
  return typeof value === "string" && BUILTIN.has(value);
}

/** Icon ids an extension action may use; free-form SVG/URLs are rejected. */
export const PET_EXTENSION_ICONS = [
  "spark",
  "heart",
  "bolt",
  "star",
  "leaf",
  "music",
  "chat",
  "moon",
  "cookie",
  "wave",
  "orbit",
  "pulse"
] as const;
export type PetExtensionIcon = (typeof PET_EXTENSION_ICONS)[number];

const ICONS = new Set<string>(PET_EXTENSION_ICONS);

export function isPetExtensionIcon(value: unknown): value is PetExtensionIcon {
  return typeof value === "string" && ICONS.has(value);
}

const MAX_ACTION_ID = 128;
const MAX_ACTION_LABEL = 48;

/**
 * Namespaced extension action id, e.g. `com.example.radio.dance`.
 * Built-in ids are deliberately not valid extension ids.
 */
export function validPetActionId(value: unknown): value is string {
  if (typeof value !== "string" || value.length < 3 || value.length > MAX_ACTION_ID) return false;
  if (!value.includes(".") || value.includes("..")) return false;
  if (value.startsWith(".") || value.endsWith(".") || value.endsWith("-") || value.endsWith("_")) return false;
  return /^[a-z0-9][a-z0-9._-]*$/.test(value);
}

/** Strips control characters and bounds the label shown in the pet menu. */
export function sanitizePetActionLabel(value: unknown): string {
  if (typeof value !== "string") return "";
  return Array.from(value)
    .filter((character) => {
      const code = character.codePointAt(0) ?? 0;
      return code >= 32 && code !== 127;
    })
    .join("")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_ACTION_LABEL);
}
