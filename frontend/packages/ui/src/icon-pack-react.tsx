import { useTheme } from "./ThemeProvider";
import type { IconName, ThemeIcon } from "./icon-pack";

/** The active theme's override for one shell icon, if it ships one. */
export function useThemeIcon(name: IconName | undefined): ThemeIcon | undefined {
  const { theme } = useTheme();
  return name ? theme.icons?.[name] : undefined;
}
