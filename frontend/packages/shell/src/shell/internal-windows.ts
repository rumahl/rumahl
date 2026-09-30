import { isInternalTarget } from "../routing/internal";
import type { ShellWindow } from "../shell-state";

const key = "rumahl.internal-windows.v1";
/** Tab-local persistence for internal tools, separate from the server workspace. */
export function readInternalWindows(): ShellWindow[] {
  try {
    const raw: unknown = JSON.parse(sessionStorage.getItem(key) ?? "[]");
    if (!Array.isArray(raw)) return [];
    return raw.filter((value): value is ShellWindow => {
      if (!value || typeof value !== "object") return false;
      const w = value as ShellWindow;
      return typeof w.location === "string" && isInternalTarget(w.location) && typeof w.minimized === "boolean" &&
        (!w.rect || [w.rect.x, w.rect.y, w.rect.width, w.rect.height].every(n => Number.isFinite(n) && n >= 0 && n <= 32768)) &&
        (!w.placement || ["floating", "left", "right", "maximized"].includes(w.placement));
    }).slice(0, 1);
  } catch { return []; }
}
export function writeInternalWindows(windows: readonly ShellWindow[]) {
  try { sessionStorage.setItem(key, JSON.stringify(windows.filter(w => w.location && isInternalTarget(w.location)))); } catch { /* Storage may be disabled. */ }
}
