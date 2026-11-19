import type { ComponentType } from "react";
import { PetOverlay } from "./pets/PetOverlay";

/**
 * Trusted first-party shell surfaces.
 *
 * A surface is rendered by the shell itself, above the desktop/launcher and
 * below the window layer. First-party apps can register companions, HUDs or
 * ambient panels here without any change to `ShellLayout`.
 *
 * Installed apps must not register React here: they run in an isolated iframe,
 * so their equivalent is a signed, declarative surface contribution that the
 * shell renders with its own components (or, later, an isolated surface
 * frame). Neither path grants shell realm access.
 */
export interface ShellSurface {
  /** Namespaced surface id, e.g. `pets.companion`. */
  id: string;
  /** The first-party app that owns the surface. */
  appId: string;
  Component: ComponentType;
}

export const shellSurfaces: readonly ShellSurface[] = [
  { id: "pets.companion", appId: "pets", Component: PetOverlay }
];
