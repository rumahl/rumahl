/**
 * The shared size scale for every control. `md` is the default and maps to the
 * baseline values of the `--control-h/-pad-x/-pad-y/-font/-icon/-gap` tokens in
 * `controls.css`; the other steps are defined there as `[data-size="…"]`.
 *
 * Components render this as a `data-size` attribute, so a single prop resizes
 * any control consistently, and themes can retune a step centrally.
 */
export type ControlSize = "xs" | "sm" | "md" | "lg" | "xl";

/** Convenience for spreading the size attribute onto an element. */
export function dataSize(size: ControlSize = "md"): { "data-size": ControlSize } {
  return { "data-size": size };
}

/** Mix into a component's props to opt into the shared size scale. */
export interface SizeProps {
  size?: ControlSize | undefined;
}
