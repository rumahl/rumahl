/**
 * Spacing helper for the layout primitives. Numbers use a 4px base scale
 * (`1` → `4px`); strings pass through as-is (`"1.5rem"`, `"8px"`).
 */
export type Spacing = number | string;

export function space(value?: Spacing): string | undefined {
  if (value === undefined) return undefined;
  return typeof value === "number" ? `${value * 4}px` : value;
}

/** A raw CSS length: numbers become pixel values. */
export type Dimension = number | string;

export function dimension(value?: Dimension): string | undefined {
  if (value === undefined) return undefined;
  return typeof value === "number" ? `${value}px` : value;
}
