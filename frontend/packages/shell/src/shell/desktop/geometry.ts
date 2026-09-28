export interface WindowRect { x: number; y: number; width: number; height: number }
export interface WorkArea { width: number; height: number }
export type WindowPlacement = "floating" | "maximized" | "left" | "right";
export function constrainRect(rect: WindowRect, area: WorkArea): WindowRect {
  const width = Math.min(Math.max(280, rect.width), Math.max(1, area.width));
  const height = Math.min(Math.max(220, rect.height), Math.max(1, area.height));
  return { width, height, x: Math.max(0, Math.min(rect.x, area.width - width)), y: Math.max(0, Math.min(rect.y, area.height - height)) };
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
