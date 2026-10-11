import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it, vi } from "vitest";
import { classicTheme, rumahlTheme } from "@rumahl/ui/themes";
import { applyTokens, clearTokens } from "@rumahl/ui/tokens";
import { rememberLoginAppearance } from "./login-appearance";

const script = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "../../../../../platform-web/src/login-appearance.js"), "utf8");
function restore() {
  applyTokens(document.documentElement, rumahlTheme.tokens);
  // Execute the exact standalone script served by the authentication gateway.
  window.eval(script);
}

afterEach(() => {
  clearTokens(document.documentElement);
  document.documentElement.removeAttribute("style");
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.scheme;
  delete document.documentElement.dataset.material;
  vi.restoreAllMocks();
});

it("restores the last active device palette and Classic presentation", () => {
  const theme = { ...classicTheme, tokens: { ...classicTheme.tokens, "color.accent": "#123456" } };
  rememberLoginAppearance(theme, "dark");
  restore();
  const root = document.documentElement;
  expect(root.dataset.theme).toBe(classicTheme.id);
  expect(root.dataset.scheme).toBe("dark");
  expect(root.dataset.material).toBe("solid");
  expect(root.style.getPropertyValue("--rumahl-ui-color-accent")).toBe("#123456");
  expect(root.style.getPropertyValue("--rumahl-ui-shape-radius-window")).toBe(classicTheme.tokens["shape.radius.window"]);
  expect(Object.keys(JSON.parse(localStorage.getItem("rumahl.login.appearance.v1")!)).sort()).toEqual(["material", "scheme", "theme", "tokens", "version"]);
});

it("keeps the standard theme with missing or corrupt storage", () => {
  for (const stored of ["null", "{", '{"version":2,"tokens":{}}']) {
    localStorage.setItem("rumahl.login.appearance.v1", stored);
    expect(restore).not.toThrow();
    expect(document.documentElement.style.getPropertyValue("--rumahl-ui-color-accent")).toBe(rumahlTheme.tokens["color.accent"]);
  }
});

it("restores bundled wallpaper but rejects external images and unknown tokens", () => {
  for (const wallpaper of ["url(/assets/monstera-abc.jpg)", "url(https://example.com/tracker.png)"]) {
    localStorage.setItem("rumahl.login.appearance.v1", JSON.stringify({ version: 1, tokens: {
      "texture.wallpaper": wallpaper,
      "unknown.token": "red"
    } }));
    restore();
    const root = document.documentElement;
    expect(root.style.getPropertyValue("--rumahl-ui-texture-wallpaper")).toBe(wallpaper.startsWith("url(/") ? wallpaper : "default");
    expect(root.style.getPropertyValue("--rumahl-ui-unknown-token")).toBe("");
  }
});

it("does not interrupt the shell when storage is disabled", () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("disabled"); });
  expect(() => rememberLoginAppearance(rumahlTheme, "light")).not.toThrow();
});
