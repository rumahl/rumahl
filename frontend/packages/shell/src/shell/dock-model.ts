import type { ShellWindow } from "../shell-state";

/** Match route boundaries, never substrings (e.g. /apps is not /app/files). */
export function windowBelongsToLink(window: ShellWindow, target: string): boolean {
  const path = window.location?.split(/[?#]/)[0];
  return path === target || (target !== "/" && path?.startsWith(`${target}/`) === true);
}
