import type { ReactNode } from "react";

export type HostArea = "apps" | "system";
export type Place = { kind: "store" } | { kind: "host"; area: HostArea } | { kind: "desktop" };

export interface StoreCrumb {
  id: string;
  name: string;
}
export interface Nav {
  place: Place;
  trail: StoreCrumb[];
  segments: string[];
}
export interface Item {
  key: string;
  name: string;
  /** Friendly label when the backing path name is an identifier. */
  label?: string;
  directory: boolean;
  size: number;
  modified: number;
  host: boolean;
  /** Desktop app shortcut: opens this path. */
  appPath?: string;
  /** Host application package or desktop app shortcut. */
  app?: boolean;
  /** Desktop app-collection folder: drill into its member apps. */
  appFolder?: boolean;
  /** Pre-rendered icon (e.g. an app icon for a shortcut). */
  icon?: ReactNode;
}

export const ROOT: StoreCrumb = { id: "root", name: "" };

export const freshStore = (): Nav => ({ place: { kind: "store" }, trail: [ROOT], segments: [] });
export const freshHost = (area: HostArea): Nav => ({ place: { kind: "host", area }, trail: [ROOT], segments: [] });
export const freshDesktop = (): Nav => ({ place: { kind: "desktop" }, trail: [], segments: [] });

export function placeLabelKey(place: Place): "files.root" | "files.applications" | "files.system" | "files.desktop" {
  if (place.kind === "store") return "files.root";
  if (place.kind === "desktop") return "files.desktop";
  return place.area === "apps" ? "files.applications" : "files.system";
}
