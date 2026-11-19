import { useEffect } from "react";
import type { Theme } from "@rumahl/ui/themes";
import type { Tokens } from "@rumahl/ui/tokens";

/** The only surface a theme's code gets. No storage, no network, no shell APIs. */
export interface ThemeRuntimeApi {
  /** The shell root (`.shell`). */
  root: HTMLElement;
  /** A layer behind windows, clipped to the wallpaper. */
  background: HTMLElement;
  /** A layer above windows for full-screen treatments. */
  overlay: HTMLElement;
  state: {
    tokens: Tokens;
    animations: boolean;
  };
}

/**
 * Executes a theme's `code` module. Themes are data plus a small module that
 * runs against this API; the module only gets the layers and tokens, so it can
 * never reach the host's privileged bridges. Returning a function from
 * `activate` registers cleanup.
 */
export function useThemeCode(
  theme: Theme,
  tokens: Tokens,
  animations: boolean,
  root: HTMLElement | null,
  background: HTMLElement | null,
  overlay: HTMLElement | null,
): void {
  // A stable dependency for the resolved tokens; themes rerun only on real
  // appearance changes, not on every render.
  const tokenKey = JSON.stringify(tokens);
  useEffect(() => {
    if (!theme.code || !root || !background || !overlay) return;
    if (typeof URL.createObjectURL !== "function") return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    const url = URL.createObjectURL(new Blob([theme.code], { type: "text/javascript" }));
    void import(/* @vite-ignore */ url)
      .then((module: { default?: unknown; activate?: unknown }) => {
        if (disposed) return;
        const activate = module.default ?? module.activate;
        if (typeof activate !== "function") return;
        const result = (activate as (api: ThemeRuntimeApi) => unknown)({
          root,
          background,
          overlay,
          state: { tokens, animations },
        });
        if (typeof result === "function") cleanup = result as () => void;
      })
      .catch((error: unknown) => {
        console.error("[theme] code failed", error);
      })
      .finally(() => URL.revokeObjectURL(url));
    return () => {
      disposed = true;
      cleanup?.();
      // Never let a broken theme leave debris behind.
      background.replaceChildren();
      overlay.replaceChildren();
    };
  }, [theme.id, theme.code, tokenKey, animations, root, background, overlay]);
}
