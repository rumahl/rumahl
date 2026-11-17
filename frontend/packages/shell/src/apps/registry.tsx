import type { RouteObject } from "react-router";
import type { MessageKey } from "../i18n/locales/en";
import { EditorApp } from "./editor/EditorApp";
import { FilesApp } from "./files/FilesApp";
import { PetsApp } from "./pets/PetsApp";
import { StoreApp } from "./StoreApp";

export interface FirstPartyApp {
  id: string;
  title: MessageKey;
  /** The app renders its own topbar into the native window titlebar. */
  customTopbar?: boolean;
  /** Keep the custom topbar in exclusive fullscreen; window controls stay hidden. */
  customTopbarFullscreen?: boolean;
  /** Preferred initial window size; the shell still clamps to the screen. */
  preferredSize?: { width: number; height: number };
  /** Settings route for first-party apps that expose one. */
  settingsPath?: string;
  routes: RouteObject[];
}
// Trusted first-party modules only. Installed app manifests never import React
// here. They need a Rust-authorized descriptor and an isolated app host.
export const firstPartyApps: readonly FirstPartyApp[] = [
  { id: "files", title: "files.title", customTopbar: true, customTopbarFullscreen: true, routes: [{ index: true, Component: FilesApp }] },
  { id: "editor", title: "editor.title", preferredSize: { width: 900, height: 620 }, routes: [{ index: true, Component: EditorApp }] },
  { id: "pets", title: "pets.title", preferredSize: { width: 1040, height: 740 }, routes: [{ index: true, Component: PetsApp }] },
  { id: "store", title: "store.title", customTopbar: true, customTopbarFullscreen: false, preferredSize: { width: 1200, height: 760 }, settingsPath: "/settings/store", routes: [{ index: true, Component: StoreApp }] }
];
export function findFirstPartyApp(id: string): FirstPartyApp | undefined {
  return firstPartyApps.find((app) => app.id === id);
}
