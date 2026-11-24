import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import {
  exportThemePackage,
  installTheme,
  installThemeFile,
  isInstalledTheme,
  isThemeTrusted,
  revokeThemeTrust,
  themeCodeHash,
  trustTheme,
  uninstallTheme
} from "./installed-themes";
import { getTheme, listThemes } from "@rumahl/ui/sdk";

/** Minimal stored-entry ZIP writer for the package import test. */
function buildZip(files: Record<string, string>): ArrayBuffer {
  const encoder = new TextEncoder();
  const chunks: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  for (const [name, content] of Object.entries(files)) {
    const nameBytes = encoder.encode(name);
    const data = encoder.encode(content);
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const view = new DataView(local.buffer);
    view.setUint32(0, 0x04034b50, true);
    view.setUint16(4, 20, true);
    view.setUint16(8, 0, true); // stored
    view.setUint32(18, data.length, true);
    view.setUint32(22, data.length, true);
    view.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);
    local.set(data, 30 + nameBytes.length);
    chunks.push(local);

    const entry = new Uint8Array(46 + nameBytes.length);
    const entryView = new DataView(entry.buffer);
    entryView.setUint32(0, 0x02014b50, true);
    entryView.setUint16(4, 20, true);
    entryView.setUint16(10, 0, true); // stored
    entryView.setUint32(20, data.length, true);
    entryView.setUint32(24, data.length, true);
    entryView.setUint16(28, nameBytes.length, true);
    entryView.setUint32(42, offset, true);
    entry.set(nameBytes, 46);
    central.push(entry);

    offset += local.length;
  }
  const centralSize = central.reduce((sum, chunk) => sum + chunk.length, 0);
  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, 0x06054b50, true);
  endView.setUint16(8, central.length, true);
  endView.setUint16(10, central.length, true);
  endView.setUint32(12, centralSize, true);
  endView.setUint32(16, offset, true);
  const total = offset + centralSize + end.length;
  const out = new Uint8Array(total);
  let cursor = 0;
  for (const chunk of [...chunks, ...central, end]) {
    out.set(chunk, cursor);
    cursor += chunk.length;
  }
  return out.buffer as ArrayBuffer;
}

describe("theme packages", () => {
  test("installs a ZIP package and inlines its stylesheet and code", async () => {
    const manifest = JSON.parse(readFileSync("../../../examples/themes/hallows/theme.json", "utf8"));
    const styles = readFileSync("../../../examples/themes/hallows/styles.css", "utf8");
    const code = readFileSync("../../../examples/themes/hallows/code.js", "utf8");
    const zip = buildZip({
      "hallows/theme.json": JSON.stringify({
        ...manifest,
        id: "com.example.zip-hallows",
        name: "ZIP Hallows",
        css: "styles.css",
        code: "code.js"
      }),
      "hallows/styles.css": styles,
      "hallows/code.js": code
    });
    const file = new File([zip], "hallows.zip", { type: "application/zip" });
    const theme = await installThemeFile(file);
    expect(theme.id).toBe("com.example.zip-hallows");
    expect(theme.css).toContain("hallows-blood");
    expect(theme.code).toContain("export default function activate");
    expect(theme.locked).toBe(true);
    expect(theme.tokens["effect.blood"]).toBe("1");
  });

  test("exports a package and uninstalls it again", async () => {
    const manifest = JSON.parse(readFileSync("../../../examples/themes/hallows/theme.json", "utf8"));
    const zip = buildZip({
      "theme.json": JSON.stringify({ ...manifest, id: "com.example.removable", name: "Removable", css: "theme.css", code: "theme.js" }),
      "theme.css": ".removable { color: red; }",
      "theme.js": "export default function activate() { return () => {}; }"
    });
    const file = new File([zip], "removable.zip", { type: "application/zip" });
    const theme = await installThemeFile(file);
    expect(isInstalledTheme(theme.id)).toBe(true);
    const exported = exportThemePackage(theme);
    expect(exported).toContain(".removable");
    expect(exported).toContain("activate");
    uninstallTheme(theme.id);
    expect(isInstalledTheme(theme.id)).toBe(false);
    expect(getTheme(theme.id)).toBeUndefined();
    expect(listThemes().some((item) => item.id === theme.id)).toBe(false);
  });

  test("remembers whether a theme came from the store or was installed manually", () => {
    const data = {
      id: "com.example.origin",
      name: "Origin",
      tokens: {
        "color.accent": "#ff5f5f",
        "color.on.accent": "#111111",
        "color.canvas.background": "#020101",
        "color.text.primary": "#ffffff",
        "color.surface": "#ffffff1a",
        "color.surface.strong": "#111111",
        "color.panel.background": "#111111"
      },
      css: "/* noop */",
      modes: ["dark"]
    };
    const store = installTheme(data, "store");
    expect(store.origin).toBe("store");
    expect(getTheme(store.id)?.origin).toBe("store");
    uninstallTheme(store.id);
    const manual = installTheme({ ...data, id: "com.example.manual" });
    expect(manual.origin).toBe("manual");
    uninstallTheme(manual.id);
  });

  test("rejects a ZIP without theme.json", async () => {
    const file = new File([buildZip({ "readme.txt": "nothing" })], "empty.zip", { type: "application/zip" });
    await expect(installThemeFile(file)).rejects.toThrow("invalid-theme");
  });

  test("keeps full-access code inactive until trusted and revokes that consent", async () => {
    const code = "export default function activate() { return () => {}; }";
    const data = {
      id: "com.example.trusted",
      name: "Trusted",
      code,
      runtime: "shell",
      tokens: {
        "color.accent": "#ff5f5f",
        "color.on.accent": "#111111",
        "color.canvas.background": "#020101",
        "color.text.primary": "#ffffff",
        "color.surface": "#ffffff1a",
        "color.surface.strong": "#111111",
        "color.panel.background": "#111111"
      },
      modes: ["dark"]
    };
    const theme = installTheme(data, "manual");
    expect(theme.execution).toBe("sandbox");
    expect(isThemeTrusted(theme)).toBe(false);
    const hash = await themeCodeHash(code);
    await trustTheme(theme.id, hash);
    const trusted = getTheme(theme.id);
    expect(trusted?.execution).toBe("trusted");
    expect(trusted && isThemeTrusted(trusted)).toBe(true);
    revokeThemeTrust(theme.id);
    const revoked = getTheme(theme.id);
    expect(revoked?.execution).toBe("sandbox");
    expect(revoked && isThemeTrusted(revoked)).toBe(false);
    uninstallTheme(theme.id);
  });
});
