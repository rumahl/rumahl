export interface WindowRect { x: number; y: number; width: number; height: number }
export interface WorkArea { width: number; height: number }
export type WindowPlacement = "floating" | "maximized" | "left" | "right";
/** How much of a window must stay inside the viewport so it stays grabbable. */
const REACHABLE = 48;
export function constrainRect(rect: WindowRect, area: WorkArea): WindowRect {
  const width = Math.min(Math.max(280, rect.width), Math.max(1, area.width));
  const height = Math.min(Math.max(220, rect.height), Math.max(1, area.height));
  // A narrow/short desktop is a small screen, not a workspace where a window
  // may hang mostly off-screen. Keep it fully visible there; on a normal
  // desktop windows may slide over an edge but keep a grabbable strip.
  const small = area.width < 700 || area.height < 500;
  const minX = small ? 0 : -(width - REACHABLE);
  const maxX = small ? Math.max(0, area.width - width) : area.width - REACHABLE;
  const minY = small ? 0 : -(height - REACHABLE);
  const maxY = small ? Math.max(0, area.height - height) : area.height - REACHABLE;
  return { width, height, x: Math.min(Math.max(rect.x, minX), maxX), y: Math.min(Math.max(rect.y, minY), maxY) };
}
export function placedRect(rect: WindowRect, placement: WindowPlacement, area: WorkArea, topInset = 0, bottomInset = 0): WindowRect {
  if (placement === "floating") return constrainRect(rect, area);
  const top = Math.min(Math.max(0, topInset), Math.max(0, area.height - 1));
  const bottom = Math.max(0, bottomInset);
  const width = placement === "maximized" ? area.width : area.width / 2;
  const height = Math.max(1, area.height - top - bottom);
  return { x: placement === "right" ? area.width - width : 0, y: top, width, height };
}
/** Rectangles as drawn by a desktop marquee selection. */
export interface SelectionRect { x: number; y: number; width: number; height: number }
export function rectsIntersect(a: SelectionRect, b: SelectionRect): boolean {
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}

/** Center a new window within the measured desktop, including small screens. */
export function centeredRect(area: WorkArea, preferred: { width: number; height: number } = { width: 760, height: 540 }): WindowRect {
  const width = Math.min(preferred.width, area.width), height = Math.min(preferred.height, area.height);
  return { x: (area.width - width) / 2, y: (area.height - height) / 2, width, height };
}
