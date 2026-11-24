import { readFileSync } from "node:fs";
import { themeFromPackage, type ThemePackageManifest } from "@rumahl/ui/themes";
import type { Theme } from "@rumahl/ui/themes";

/** Loads one of the example theme packages exactly like an installed ZIP. */
export function loadThemePackage(name: "classic" | "hallows"): Theme {
  const directory = `${process.cwd()}/../../../examples/themes/${name}`;
  const manifest = JSON.parse(readFileSync(`${directory}/theme.json`, "utf8")) as ThemePackageManifest;
  let code: string | undefined;
  try {
    code = readFileSync(`${directory}/code.js`, "utf8");
  } catch {
    code = undefined;
  }
  return themeFromPackage(manifest, readFileSync(`${directory}/styles.css`, "utf8"), code);
}
