import { useEffect } from "react";
import type { Theme } from "@rumahl/ui/themes";
import type { Tokens } from "@rumahl/ui/tokens";
import { mountSandboxTheme } from "./theme-sandbox";

/**
 * Full-access API for a theme the user explicitly trusted. This is the legacy
 * shell-realm runtime, not a security boundary; use the sandboxed layer API
 * (`ThemeSandboxApi` from `@rumahl/ui/sdk`) for untrusted code.
 */
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
 * Applies a theme's code according to its declared support and the user's
 * local execution choice:
 *
 * - trusted `shell`/`both` code uses the full-access module API;
 * - `sandbox`/`both` code draws inside private iframe layers;
 * - `shell`-only code stays inactive until the user explicitly trusts it.
 *
 * Returning a function from `activate` registers cleanup.
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
  const runtime = theme.runtime ?? "shell";
  const trusted = theme.execution === "trusted"
    && typeof theme.codeHash === "string"
    && theme.trustedHash === theme.codeHash;
  useEffect(() => {
    if (!theme.code || !root || !background || !overlay) return;
    if (trusted && runtime !== "sandbox") {
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
    }
    if (runtime !== "shell") {
      return mountSandboxTheme(theme, tokens, animations, background, overlay);
    }
    // Full access was not granted (or the code changed): keep the layers empty.
    background.replaceChildren();
    overlay.replaceChildren();
    return undefined;
  }, [theme, theme.id, theme.code, theme.runtime, theme.execution, theme.codeHash, theme.trustedHash, trusted, runtime, tokens, tokenKey, animations, root, background, overlay]);
}
