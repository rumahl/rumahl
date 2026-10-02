import { describe, expect, test } from "vitest";
import { paletteTokens } from "@rumahl/ui/palette";
import { demoSnapshot } from "../../shell/src/demo/snapshot";
import { renderShellDocument } from "./render";

const assets = {
  script: "/assets/index-abc123.js",
  stylesheet: "/assets/index-abc123.css"
};
const nonce = "AbCdEfGhIjKlMnOpQrStUvWx";

async function documentFor(snapshot: unknown, requestPath = "/"): Promise<string> {
  const { stream } = await renderShellDocument(snapshot, assets, nonce, requestPath);
  let document = "";
  for await (const chunk of stream) document += chunk.toString();
  return document;
}

describe("server-rendered shell", () => {
  test("embeds the exact snapshot without executable HTML in JSON", async () => {
    const maliciousTitle = "</script><script>alert(1)</script>";
    const snapshot = {
      ...demoSnapshot,
      contributions: [
        { ...demoSnapshot.contributions[0], title: maliciousTitle },
        ...demoSnapshot.contributions.slice(1)
      ]
    };
    const html = await documentFor(snapshot);

    expect(html).toContain("data-shell-ssr=\"1\"");
    expect(html).toContain(`id="rumahl-shell-snapshot"`);
    expect(html).toContain("\\u003c/script\\u003e");
    expect(html).not.toContain(maliciousTitle);
  });

  test("renders the requested route with a deterministic mode before storage hydration", async () => {
    const desktop = await documentFor(demoSnapshot, "/app/app-manager");
    expect(desktop).toContain('data-window-id="app:app-manager"');
    const launcher = await documentFor(demoSnapshot, "/settings/display?mode=launcher");
    expect(launcher).toContain('data-shell-mode="desktop"');
    expect(launcher).toContain('data-window-id="/settings/*"');
    const unknown = await documentFor(demoSnapshot, "/app/not-installed");
    expect(unknown).toContain("App unavailable");
    for (const path of ["//foreign.test", "https://foreign.test", "/x\\y", "/x\n"]) {
      await expect(documentFor(demoSnapshot, path)).rejects.toThrow("invalid shell request path");
    }
  });

  test("server-renders installed apps from the snapshot", async () => {
    const html = await documentFor({
      ...demoSnapshot,
      apps: [{ id: "com.example.notes", title: "Notes", launchable: true }]
    }, "/");
    expect(html).toContain("Notes");
  });

  test("server-renders restored workspace windows with a nonce position style", async () => {
    const html = await documentFor({
      ...demoSnapshot,
      workspace: JSON.stringify({
        version: 1,
        folders: [],
        windows: [{ location: "/settings/display", rect: { x: 40, y: 30, width: 700, height: 500 }, placement: "floating", minimized: false }]
      })
    }, "/");
    expect(html).toContain('data-window-id="/settings/*"');
    expect(html).toContain('width:700px');
    expect(html).toContain('nonce="AbCdEfGhIjKlMnOpQrStUvWx"');
  });

  test("server-renders the stored desktop arrangement", async () => {
    const html = await documentFor({
      ...demoSnapshot,
      apps: [{ id: "com.example.notes", title: "Notes", launchable: true }],
      workspace: JSON.stringify({
        version: 1,
        folders: [],
        windows: [],
        desktop: { order: ["com.example.notes"], hidden: [], widgets: true },
        launcherView: null
      })
    }, "/");
    const notes = html.indexOf("Notes");
    const files = html.indexOf(">Files<");
    expect(notes).toBeGreaterThanOrEqual(0);
    expect(files).toBeGreaterThanOrEqual(0);
    expect(notes).toBeLessThan(files);
  });

  test("server-renders the device appearance (dark mode)", async () => {
    const html = await documentFor({
      ...demoSnapshot,
      workspace: JSON.stringify({
        version: 1,
        folders: [],
        windows: [],
        desktop: { order: [], hidden: [], widgets: true },
        launcherView: null,
        appearance: { seed: "#e5484d", mode: "dark", tokens: {} }
      })
    }, "/");
    const dark = paletteTokens("#e5484d", { mode: "dark" });
    expect(html).toContain(`--rumahl-ui-color-surface-strong:${dark["color.surface.strong"]}`);
  });

  test("isolates consecutive users and rejects mismatched builds", async () => {
    const first = await documentFor(demoSnapshot);
    const second = await documentFor({
      ...demoSnapshot,
      user: { displayName: "Lea", locale: "de-DE" }
    });

    expect(first).toContain("Kaim");
    expect(second).toContain("Lea");
    expect(second).not.toContain("Kaim");
    expect(second).toContain('lang="de"');
    await expect(
      documentFor({ ...demoSnapshot, shellBuildId: "incompatible-build" })
    ).rejects.toThrow("incompatible shell build");
  });
});
