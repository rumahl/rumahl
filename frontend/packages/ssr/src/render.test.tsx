import { describe, expect, test } from "vitest";
import { demoSnapshot } from "../../shell/src/demo/snapshot";
import { renderShellDocument } from "./render";

const assets = {
  script: "/assets/index-abc123.js",
  stylesheet: "/assets/index-abc123.css"
};
const nonce = "AbCdEfGhIjKlMnOpQrStUvWx";

async function documentFor(snapshot: unknown): Promise<string> {
  const { stream } = await renderShellDocument(snapshot, assets, nonce);
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
