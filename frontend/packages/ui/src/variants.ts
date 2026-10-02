/**
 * Presentation variants select the *structure* of the shell. They are
 * implemented by trusted first-party code, while a theme only chooses which
 * variant is active. Unknown values fall back to the rumahl defaults so a
 * malformed theme can never leave the shell without a layout.
 */
export const SHELL_LAYOUTS = ["dock", "taskbar"] as const;
export type ShellLayoutId = (typeof SHELL_LAYOUTS)[number];

export const LAUNCHER_LAYOUTS = ["springboard", "drawer"] as const;
export type LauncherLayoutId = (typeof LAUNCHER_LAYOUTS)[number];

export const WINDOW_CHROME = ["standard", "compact"] as const;
export type WindowChromeId = (typeof WINDOW_CHROME)[number];

export interface VariantSelection {
  shellLayout: ShellLayoutId;
  launcherLayout: LauncherLayoutId;
  windowChrome: WindowChromeId;
}

export const defaultVariants: VariantSelection = {
  shellLayout: "dock",
  launcherLayout: "springboard",
  windowChrome: "standard"
};

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value) ? (value as T) : fallback;
}

/** Validates untrusted variant data (for example from a theme package). */
export function sanitizeVariants(value: unknown): VariantSelection {
  const input = (typeof value === "object" && value ? value : {}) as Record<string, unknown>;
  return {
    shellLayout: pick(input.shellLayout, SHELL_LAYOUTS, defaultVariants.shellLayout),
    launcherLayout: pick(input.launcherLayout, LAUNCHER_LAYOUTS, defaultVariants.launcherLayout),
    windowChrome: pick(input.windowChrome, WINDOW_CHROME, defaultVariants.windowChrome)
  };
}

export function resolveVariants(partial?: Partial<VariantSelection> | undefined): VariantSelection {
  return sanitizeVariants({ ...defaultVariants, ...partial });
}
