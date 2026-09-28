import { useShell } from "../shell/ShellContext";
import { DesktopHome } from "../shell/desktop/DesktopHome";
import { LauncherHome } from "../shell/launcher/LauncherHome";
export function HomePage() { return useShell().mode === "desktop" ? <DesktopHome /> : <LauncherHome />; }
