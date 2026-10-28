import { useMemo } from "react";
import { useI18n } from "../i18n";
import { appPath } from "../routing/paths";
import { useAppCatalog } from "./AppCatalog";
import { firstPartyApps } from "./registry";
import { useDisabledApps } from "../preferences/disabled-apps";

export interface ShellApp {
  id: string;
  title: string;
  path: string;
  launchable: boolean;
  system: boolean;
  kind: "settings" | "app";
  /** Optional app-provided icon image (URL); falls back to a built-in glyph. */
  icon?: string;
  /** Installation identifier for installed apps. */
  installationId?: string;
  /** Runtime kind exposed by the catalog, when known. */
  runtime?: "web" | "native" | "container";
  /** System-level app management inside Settings. */
  managePath?: string;
  /** The app's own settings page; only present when one is exposed. */
  settingsPath?: string;
}

/**
 * Single source of truth for the app tiles shown by the desktop, launcher,
 * start menu and app catalogue. Installed apps stay untrusted descriptors
 * until the Rust-authorized catalog exposes them.
 */
export function useShellApps(options: { includeDisabled?: boolean } = {}): readonly ShellApp[] {
  const { t } = useI18n();
  const catalog = useAppCatalog();
  const disabled = useDisabledApps();
  // Until the authorized catalog is fetched, render the server snapshot's apps
  // so the desktop/launcher are complete on the first paint.
  const installed = catalog.status === "ready" ? catalog.apps : catalog.preview;
  return useMemo(() => ([
    ...firstPartyApps.map((app): ShellApp => ({
      id: app.id, title: t(app.title), path: appPath(app.id), launchable: true, system: true, kind: "app",
      ...(app.settingsPath ? { settingsPath: app.settingsPath } : {})
    })),
    { id: "settings", title: t("nav.settings"), path: "/settings", launchable: true, system: true, kind: "settings" as const },
    ...installed
      .filter((app) => !firstPartyApps.some((system) => system.id === app.id))
      .map((app): ShellApp => ({
        id: app.id, title: app.title, path: appPath(app.id), launchable: app.launchable, system: false, kind: "app",
        ...(app.installationId ? { installationId: app.installationId } : {}),
        ...(app.runtime ? { runtime: app.runtime } : {}),
        managePath: `/settings/apps/${encodeURIComponent(app.id)}`
      }))
  ]).filter((app) => options.includeDisabled || !disabled.includes(app.id)), [t, installed, disabled, options.includeDisabled]);
}
