import type { WeatherWarning } from "./warnings-client";

/** How long a device must be idle before an extreme warning takes over the screen. */
export const EXTREME_WARNING_IDLE_MS = 45_000;

const EXTREME_SEVERITY = /^(extreme|extraordinary)$/i;
const EXTREME_EVENT = /tornado|tsunami|katastroph|lebensgefahr|life[- ]threatening/i;

/**
 * Only warnings that are truly life-threatening may take over the whole screen.
 * Regular emergency notifications and every weather warning still use the
 * topbar; test warnings never auto-open fullscreen unless explicitly marked
 * extreme by the developer test source.
 */
export function isExtremeWeatherWarning(warning: WeatherWarning): boolean {
  if (EXTREME_SEVERITY.test(warning.severity.trim())) return true;
  if (warning.test) return false;
  if (warning.level === "emergency") return true;
  return warning.level === "critical" && EXTREME_EVENT.test(`${warning.event} ${warning.headline}`);
}
