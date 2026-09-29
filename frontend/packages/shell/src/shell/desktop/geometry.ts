export interface WindowRect { x: number; y: number; width: number; height: number }
export interface WorkArea { width: number; height: number }
export type WindowPlacement = "floating" | "maximized" | "left" | "right";
/** How much of a window must stay inside the viewport so it stays grabbable. */
const REACHABLE = 48;
export function constrainRect(rect: WindowRect, area: WorkArea): WindowRect {
  const width = Math.min(Math.max(280, rect.width), Math.max(1, area.width));
  const height = Math.min(Math.max(220, rect.height), Math.max(1, area.height));
  // Windows may slide over any edge (including under the dock) but keep a strip
  // on screen so the title bar stays draggable.
  const minX = -(width - REACHABLE), maxX = area.width - REACHABLE;
  const minY = -(height - REACHABLE), maxY = area.height - REACHABLE;
  return { width, height, x: Math.min(Math.max(rect.x, minX), maxX), y: Math.min(Math.max(rect.y, minY), maxY) };
}
export function placedRect(rect: WindowRect, placement: WindowPlacement, area: WorkArea): WindowRect {
  if (placement === "floating") return constrainRect(rect, area);
  const width = placement === "maximized" ? area.width : area.width / 2;
  return { x: placement === "right" ? area.width - width : 0, y: 0, width, height: area.height };
}
/** Rectangles as drawn by a desktop marquee selection. */
export interface SelectionRect { x: number; y: number; width: number; height: number }
export function rectsIntersect(a: SelectionRect, b: SelectionRect): boolean {
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}
