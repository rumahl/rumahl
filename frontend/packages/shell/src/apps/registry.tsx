import type { RouteObject } from "react-router";
import type { MessageKey } from "../i18n/locales/en";
import { FilesApp } from "./files/FilesApp";
import { AppManager } from "./AppManager";

export interface FirstPartyApp {
  id: string;
  title: MessageKey;
  routes: RouteObject[];
}
// Trusted first-party modules only. Installed app manifests never import React
// here. They need a Rust-authorized descriptor and an isolated app host.
export const firstPartyApps: readonly FirstPartyApp[] = [
  { id: "files", title: "files.title", routes: [{ index: true, Component: FilesApp }] },
  { id: "app-manager", title: "appManager.title", routes: [{ index: true, Component: AppManager }] }
];
export function findFirstPartyApp(id: string): FirstPartyApp | undefined {
  return firstPartyApps.find((app) => app.id === id);
}
