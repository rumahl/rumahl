import type { Theme } from "@rumahl/ui/themes";

/** Device-local presentation for the unauthenticated screen. No account data. */
export function rememberLoginAppearance(theme: Theme, scheme: "light" | "dark"): void {
  try {
    // Theme colour tokens are public presentation data, not credentials.
    // codeql[js/clear-text-storage-of-sensitive-data]
    localStorage.setItem("rumahl.login.appearance.v1", JSON.stringify({
      version: 1,
      theme: theme.id,
      scheme,
      material: theme.material ?? (Number(theme.tokens["material.morphism"]) === 0 ? "solid" : "translucent"),
      tokens: theme.tokens
    }));
  } catch {
    // Disabled storage must never prevent sign-in or theme changes.
  }
}
