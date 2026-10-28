import type { WindowRect } from "./desktop/geometry";

const key = (id: string) => `rumahl.window-position.v1:${id}`;
export function readWindowRect(id: string): WindowRect | undefined {
  try {
    const value = JSON.parse(localStorage.getItem(key(id)) ?? "null");
    if (value && [value.x, value.y, value.width, value.height].every(v => typeof v === "number" && Number.isFinite(v)) && value.width > 0 && value.height > 0) {
      return { x: value.x, y: value.y, width: value.width, height: value.height };
    }
  } catch { /* Storage may be unavailable or contain invalid data. */ }
  return undefined;
}
export function writeWindowRect(id: string, rect: WindowRect): void {
  try { localStorage.setItem(key(id), JSON.stringify(rect)); } catch { /* Keep the current session usable. */ }
}
