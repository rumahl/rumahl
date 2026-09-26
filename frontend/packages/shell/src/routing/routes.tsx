import type { ReactNode } from "react";
import { matchRoutes, useRoutes, type RouteObject } from "react-router";
import { findFirstPartyApp } from "../apps/registry";
import { SectionPlaceholder } from "../components/SectionPlaceholder";
import { SettingsPage } from "../pages/SettingsPage";
import { HomePage } from "../pages/HomePage";
import { AppsPage } from "../pages/AppsPage";
import { AppPage } from "../pages/AppPage";
import { StreamPage } from "../pages/StreamPage";
import { UnavailablePage } from "../pages/UnavailablePage";
import type { ShellSection } from "../shell-state";
import type { MessageKey } from "../i18n/locales/en";

export interface ShellRouteHandle {
  section: ShellSection;
  title: MessageKey;
  presentation: "page" | "window";
}
function route(path: string, element: ReactNode, handle: ShellRouteHandle): RouteObject {
  return { id: path, path, element, handle };
}
// Add trusted shell pages here; each app owns its own nested route table.
// API, login, recovery and untrusted app resources are not shell routes.
export const shellRoutes: RouteObject[] = [
  route("/", <HomePage />, { section: "home", title: "nav.home", presentation: "page" }),
  route("/apps", <AppsPage />, { section: "apps", title: "nav.apps", presentation: "page" }),
  route("/activity", <SectionPlaceholder section="activity" />, { section: "activity", title: "nav.activity", presentation: "window" }),
  route("/settings/*", <SettingsPage />, { section: "settings", title: "nav.settings", presentation: "window" }),
  route("/app/:appId/*", <AppPage />, { section: "apps", title: "route.appUnavailable", presentation: "window" }),
  route("/streams/:sessionId", <StreamPage />, { section: "apps", title: "stream.subtitle", presentation: "window" }),
  route("*", <UnavailablePage />, { section: "home", title: "route.notFound", presentation: "page" })
];
export function describeRoute(location: string) {
  const url = new URL(location, "https://shell.invalid");
  const match = matchRoutes(shellRoutes, url.pathname)!.at(-1)!;
  const handle = match.route.handle as ShellRouteHandle;
  const app = match.params.appId ? findFirstPartyApp(match.params.appId) : undefined;
  return {
    appId: match.params.appId,
    ...handle, title: app?.title ?? handle.title,
    id: match.params.appId ? `app:${match.params.appId}` : match.params.sessionId ? `stream:${match.params.sessionId}` : match.route.id!,
    stream: match.params.sessionId !== undefined,
    streamTitle: match.params.sessionId ? url.searchParams.get("title")?.slice(0, 128) : undefined
  };
}
export function ShellRoutes({ location }: { location?: string }) {
  return useRoutes(shellRoutes, location);
}
