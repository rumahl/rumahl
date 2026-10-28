import { useParams, useRoutes } from "react-router";
import { InstalledAppHost } from "../apps/InstalledAppHost";
import { InstalledAppSettings } from "../apps/AppSettings";
import { findFirstPartyApp } from "../apps/registry";
import { APP_ID } from "../routing/paths";
import { UnavailablePage } from "./UnavailablePage";
export function AppPage() {
  const { appId = "" } = useParams();
  const app = APP_ID.test(appId) ? findFirstPartyApp(appId) : undefined;
  return useRoutes([
    ...(app?.routes ?? []),
    ...(app ? [] : [{ path: "settings", element: <InstalledAppSettings /> }]),
    { path: "*", element: app ? <UnavailablePage /> : <InstalledAppHost /> }
  ]);
}
