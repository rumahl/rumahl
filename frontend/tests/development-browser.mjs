// Run only against the disposable frontend copy created by the Python harness.
import assert from "node:assert/strict";
import console from "node:console";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile, appendFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const state = process.argv[2];
const frontend = dirname(dirname(fileURLToPath(import.meta.url)));
const info = JSON.parse(await readFile(join(state, "server.json"), "utf8"));
const credentials = JSON.parse(await readFile(join(state, "credentials.json"), "utf8"));
// Trust only this test certificate's public key, never all invalid certificates.
const publicKey = execFileSync("openssl", ["x509", "-in", info.certificate, "-pubkey", "-noout"]);
const der = execFileSync("openssl", ["pkey", "-pubin", "-outform", "DER"], { input: publicKey });
const fingerprint = createHash("sha256").update(der).digest("base64");
const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${fingerprint}`] });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  await page.goto(`${info.origin}/login`);
  // Exercise a native form POST: the browser must supply its real Origin.
  await page.locator("form").evaluate((form, login) => {
    form.elements.username.value = login.username;
    form.elements.password.value = login.password;
  }, credentials);
  const hydrated = page.waitForResponse((response) =>
    response.url() === `${info.origin}/api/v1/shell/snapshot` && response.status() === 200);
  await page.locator('button[type="submit"]').click();
  await page.locator(".shell").waitFor();
  await hydrated;
  await page.locator(".search-trigger").click();
  await page.locator(".command-palette").waitFor();
  await page.evaluate(() => { globalThis.__rumahlRefreshProbe = "preserved"; });

  const app = join(frontend, "packages/shell/src/App.tsx");
  const source = await readFile(app, "utf8");
  assert(source.includes('data-dev-probe="changed"'), "expected disposable test fixture");
  await writeFile(app, source.replace('data-dev-probe="changed"', 'data-dev-probe="browser-hot"'));
  await page.locator('[data-dev-probe="browser-hot"]').waitFor();
  assert.equal(await page.evaluate(() => globalThis.__rumahlRefreshProbe), "preserved");
  assert.equal(await page.locator(".command-palette").count(), 1, "React state lost during refresh");

  await appendFile(join(frontend, "packages/shell/src/styles.css"), "\n.shell { outline: 3px solid rgb(1, 2, 3); }\n");
  await page.waitForFunction(() => globalThis.getComputedStyle(globalThis.document.querySelector(".shell")).outlineColor === "rgb(1, 2, 3)");
  assert.deepEqual(errors, [], "browser console/hydration errors");
  console.log("PASS: Chromium form login, hydration, React state preservation and CSS hot reload");
} finally {
  await browser.close();
}
