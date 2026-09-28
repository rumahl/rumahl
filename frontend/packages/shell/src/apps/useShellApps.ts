import { useMemo } from "react";
import { useI18n } from "../i18n";
import { appPath } from "../routing/paths";
import { useAppCatalog } from "./AppCatalog";
import { firstPartyApps } from "./registry";

export interface ShellApp {
  id: string;
  title: string;
  path: string;
  launchable: boolean;
  system: boolean;
  kind: "settings" | "app";
}

/**
 * Single source of truth for the app tiles shown by the desktop, launcher,
 * start menu and app catalogue. Installed apps stay untrusted descriptors
 * until the Rust-authorized catalog exposes them.
 */
export function useShellApps(): readonly ShellApp[] {
  const { t } = useI18n();
  const catalog = useAppCatalog();
  // Until the authorized catalog is fetched, render the server snapshot's apps
  // so the desktop/launcher are complete on the first paint.
  const installed = catalog.status === "ready" ? catalog.apps : catalog.preview;
  return useMemo(() => [
    ...firstPartyApps.map((app): ShellApp => ({
      id: app.id, title: t(app.title), path: appPath(app.id), launchable: true, system: true, kind: "app"
    })),
    { id: "settings", title: t("nav.settings"), path: "/settings", launchable: true, system: true, kind: "settings" },
    ...installed
      .filter((app) => !firstPartyApps.some((system) => system.id === app.id))
      .map((app): ShellApp => ({
        id: app.id, title: app.title, path: appPath(app.id), launchable: app.launchable, system: false, kind: "app"
      }))
  ], [t, installed]);
}
