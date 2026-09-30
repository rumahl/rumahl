/** Registered internal destinations; never added to the normal app catalogue. */
export const DESIGN_URI = "rumahl://design";
export function isInternalTarget(target: string): boolean { return target === DESIGN_URI; }
export function shellLocation(location: string): string {
  if (isInternalTarget(location)) return location;
  const hash = location.indexOf("#");
  const fragment = hash >= 0 ? location.slice(hash + 1) : "";
  return isInternalTarget(fragment) ? fragment : location;
}
