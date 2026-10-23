import type { CSSProperties } from "react";
import { bestContrast } from "./color";

/** Mix into a component's props to opt into a per-element custom colour. */
export interface ColorProps {
  /** Overrides the accent (primary) colour for this element only. */
  color?: string | undefined;
}

/**
 * Readable foreground for a background colour, chosen by WCAG contrast ratio
 * (whichever of ink/white reaches AA first). Falls back to white on bad input.
 */
export function onColor(background: string): string {
  try {
    return bestContrast(["#0b0d12", "#ffffff"], background);
  } catch {
    return "#ffffff";
  }
}

/**
 * Inline custom properties that re-point `--rumahl-ui-color-accent` (and the
 * readable on-accent colour) for a single element, so any component can take a
 * custom colour without changing the theme.
 */
export function colorStyle(color?: string): CSSProperties {
  if (!color) return {};
  return {
    "--rumahl-ui-color-accent": color,
    "--rumahl-ui-color-on-accent": onColor(color)
  } as CSSProperties;
}
