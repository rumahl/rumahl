import type { RouteObject } from "react-router";
import type { MessageKey } from "../i18n/locales/en";
import { FilesApp } from "./files/FilesApp";
import { StoreApp } from "./StoreApp";

export interface FirstPartyApp {
  id: string;
  title: MessageKey;
  /** The app renders its own topbar into the native window titlebar. */
  customTopbar?: boolean;
  /** Preferred initial window size; the shell still clamps to the screen. */
  preferredSize?: { width: number; height: number };
  /** Settings route for first-party apps that expose one. */
  settingsPath?: string;
  routes: RouteObject[];
}
// Trusted first-party modules only. Installed app manifests never import React
// here. They need a Rust-authorized descriptor and an isolated app host.
export const firstPartyApps: readonly FirstPartyApp[] = [
  { id: "files", title: "files.title", customTopbar: true, routes: [{ index: true, Component: FilesApp }] },
  { id: "store", title: "store.title", customTopbar: true, preferredSize: { width: 1200, height: 760 }, settingsPath: "/settings/store", routes: [{ index: true, Component: StoreApp }] }
];
export function findFirstPartyApp(id: string): FirstPartyApp | undefined {
  return firstPartyApps.find((app) => app.id === id);
}
