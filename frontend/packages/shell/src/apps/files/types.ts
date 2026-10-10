export type HostArea = "apps" | "system";
export type Place = { kind: "store" } | { kind: "host"; area: HostArea };

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
  directory: boolean;
  size: number;
  modified: number;
  host: boolean;
}

export const ROOT: StoreCrumb = { id: "root", name: "" };

export const freshStore = (): Nav => ({ place: { kind: "store" }, trail: [ROOT], segments: [] });
export const freshHost = (area: HostArea): Nav => ({ place: { kind: "host", area }, trail: [ROOT], segments: [] });

export function placeLabelKey(place: Place): "files.root" | "files.applications" | "files.system" {
  if (place.kind === "store") return "files.root";
  return place.area === "apps" ? "files.applications" : "files.system";
}
