import { describe, expect, test } from "vitest";
import { injectSplash } from "../scripts/splash.js";
import { renderShellDocument } from "../../ssr/src/render";
import { demoSnapshot } from "./demo/snapshot";

const transform = (html: string, filename = "index.html") => injectSplash.transformIndexHtml(html, { filename });

describe("splash HTML injection", () => {
  test("adds one splash to a client document, even when transformed again", () => {
    const html = transform('<html><head></head><body><div id="root"></div></body></html>');
    expect(html.match(/id="rumahl-splash"/g)).toHaveLength(1);
    expect(transform(html)).toBe(html);
  });

  test("preserves the SSR splash and its nonce in the development gateway", async () => {
    const { stream } = await renderShellDocument(demoSnapshot, {
      script: "/assets/index-test.js", stylesheet: "/assets/index-test.css"
    }, "AbCdEfGhIjKlMnOpQrStUvWx");
    let html = "";
    for await (const chunk of stream) html += chunk.toString();
    const transformed = transform(html);
    expect(transformed.match(/id="rumahl-splash"/g)).toHaveLength(1);
    expect(transformed).toBe(html);
  });

  test.each(["demo-widget.html", "404.html"])("leaves %s without a splash", (filename) => {
    const html = "<html><head></head><body></body></html>";
    expect(transform(html, filename)).toBe(html);
  });
});
