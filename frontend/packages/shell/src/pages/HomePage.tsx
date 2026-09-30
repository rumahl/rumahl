import { useTheme } from "@rumahl/ui";
import { useShell } from "../shell/ShellContext";
import { DesktopHome } from "../shell/desktop/DesktopHome";
import { LauncherHome } from "../shell/launcher/LauncherHome";
import { SlotRegion } from "../shell/SlotRegion";

/** Themes may replace the desktop/launcher content with their own slot markup. */
export function HomePage() {
  const { mode } = useShell();
  const { theme } = useTheme();
  if (mode === "desktop") return theme.slots?.desktop ? <SlotRegion name="desktop" /> : <DesktopHome />;
  return theme.slots?.launcher ? <SlotRegion name="launcher" /> : <LauncherHome />;
}
