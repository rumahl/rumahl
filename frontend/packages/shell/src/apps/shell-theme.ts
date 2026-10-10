import type { BridgeTheme } from "@rumahl/contracts/bridge";

/**
 * Reads the current shell appearance (scheme + semantic palette) from the theme
 * tokens and the scene. Shared by the app bridge and the stream theme bridge.
 */
export function readShellTheme(tokens: Record<string, string>): BridgeTheme {
  const scene = typeof document === "undefined" ? null : document.querySelector<HTMLElement>(".scene");
  const scheme = scene?.dataset.scheme
    ?? (typeof document !== "undefined" && document.body.classList.contains("light") ? "light" : "dark");
  const token = (key: string) => String(tokens[key] ?? "");
  return {
    scheme: scheme === "light" ? "light" : "dark",
    accent: token("color.accent"),
    reducedMotion:
      typeof window !== "undefined" && typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    palette: {
      background: token("color.canvas.background"),
      surface: token("color.surface"),
      text: token("color.text.primary"),
      textMuted: token("color.text.muted"),
      border: token("color.outline"),
      accent: token("color.accent"),
      onAccent: token("color.on.accent")
    }
  };
}
