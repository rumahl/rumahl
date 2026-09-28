import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { getTheme } from "@rumahl/ui/sdk";
import { applyTokens, defaultTokens, type Tokens } from "@rumahl/ui/tokens";
import type { Theme } from "@rumahl/ui/themes";
import wallpaperUrl from "./assets/monstera.jpg";
import { emptyAppearance, parseWorkspace } from "./preferences/Workspace";
import { applyTuning } from "./preferences/theme-tuning";

/** Builds the server-resolved theme, substituting the bundled wallpaper. */
export function buildSnapshotTheme(snapshot: ShellSnapshotV1): Theme {
  const base: Tokens = { ...defaultTokens, ...(snapshot.theme.tokens as Partial<Tokens>) };
  const tokens = base["texture.wallpaper"] === "default"
    ? { ...base, "texture.wallpaper": `url(${wallpaperUrl})` }
    : base;
  return {
    id: snapshot.theme.id,
    name: snapshot.theme.id,
    tokens,
    variants: {
      shellLayout: snapshot.theme.shellLayout,
      launcherLayout: snapshot.theme.launcherLayout,
      windowChrome: snapshot.theme.windowChrome
    },
    parameters: getTheme(snapshot.theme.id)?.parameters ?? [],
    modes: getTheme(snapshot.theme.id)?.modes ?? ["light"]
  };
}

/** A user/device theme selection that differs from the server theme wins locally. */
export function resolveActiveTheme(snapshotTheme: Theme, preferencesTheme: string): Theme {
  return preferencesTheme === snapshotTheme.id ? snapshotTheme : getTheme(preferencesTheme) ?? snapshotTheme;
}

/** Resolves the wallpaper sentinel after a theme's tokens were customized. */
export function resolveTokens(tokens: Tokens): Tokens {
  return tokens["texture.wallpaper"] === "default"
    ? { ...tokens, "texture.wallpaper": `url(${wallpaperUrl})` }
    : tokens;
}

/** Parses the device appearance customization embedded in the snapshot. */
export function tuningFromSnapshot(snapshot: ShellSnapshotV1) {
  if (!snapshot.workspace) return emptyAppearance;
  try {
    return parseWorkspace(JSON.parse(snapshot.workspace)).appearance;
  } catch {
    return emptyAppearance;
  }
}

/**
 * Applies the resolved appearance to the document before hydration so the first
 * paint already shows the account theme plus the saved device appearance — no
 * flash of the default theme.
 */
export function preloadAppearance(snapshot: ShellSnapshotV1): void {
  if (typeof document === "undefined") return;
  const serverTheme = buildSnapshotTheme(snapshot);
  const active = resolveActiveTheme(serverTheme, snapshot.theme.id);
  applyTokens(document.documentElement, resolveTokens(applyTuning(active.tokens, tuningFromSnapshot(snapshot))));
}
