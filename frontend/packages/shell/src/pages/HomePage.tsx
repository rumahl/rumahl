import { AppsPage } from "./AppsPage";
import { Dashboard } from "../components/Dashboard";
import { appPath } from "../routing/paths";
import { useShell } from "../shell/ShellContext";
export function HomePage() {
  const { snapshot, live, open, mode } = useShell();
  if (mode === "launcher") return <AppsPage />;
  return <Dashboard contributions={snapshot.contributions} displayName={snapshot.user.displayName}
    revision={snapshot.revision} onOpenApps={() => open(appPath("app-manager"))}
    systemStatus={snapshot.systemStatus} widgetRequest={live?.request}
    allowDevelopmentLoopback={live?.allowDevelopmentLoopback} />;
}
