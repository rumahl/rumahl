// Run only against the disposable frontend copy created by the Python harness.
import { Buffer } from "node:buffer";
import assert from "node:assert/strict";
import console from "node:console";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile, appendFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import process from "node:process";
import { URL, fileURLToPath } from "node:url";
import { request as httpsRequest } from "node:https";
import { chromium } from "playwright";

async function assetStatus(url, certificate) {
  const ca = await readFile(certificate);
  return new Promise((resolve, reject) => {
    const request = httpsRequest(url, { ca, lookup: (_hostname, options, callback) => options.all ? callback(null, [{ address: "127.0.0.1", family: 4 }]) : callback(null, "127.0.0.1", 4) }, (response) => {
      response.resume(); resolve(response.statusCode);
    });
    request.on("error", reject); request.end();
  });
}
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
  async function setMode(value, target = page) {
    await target.waitForFunction(() => !globalThis.document.querySelector('select[aria-label="Shell mode"]').disabled);
    if (await target.getByRole("combobox", { name: "Shell mode" }).inputValue() !== value) {
      const saved = target.waitForResponse((response) => response.url().includes("/api/v1/shell/preferences?") && response.request().method() === "PUT");
      await target.getByRole("combobox", { name: "Shell mode" }).selectOption(value);
      assert.equal((await saved).status(), 200);
    }
    await target.locator(`.shell[data-shell-mode="${value}"]`).waitFor();
    assert.equal(new URL(target.url()).searchParams.has("mode"), false);
  }
  async function synchronized(target, source) {
    const revision = Number(await source.locator(".shell").getAttribute("data-preferences-revision"));
    await target.waitForFunction((revision) => Number(globalThis.document.querySelector(".shell")?.dataset.preferencesRevision) >= revision, revision);
  }
  async function login(target, path = "/") {
    await target.goto(`${info.origin}${path}`);
    await target.locator("form").evaluate((form, login) => {
      form.elements.username.value = login.username; form.elements.password.value = login.password;
    }, credentials);
    await target.locator('button[type="submit"]').click();
    await target.locator(".shell").waitFor();
    await target.waitForFunction(() => !globalThis.document.querySelector('select[aria-label="Shell mode"]').disabled);
  }
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  await page.goto(`${info.origin}/app/app-manager?mode=launcher`);
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
  assert.equal(new URL(page.url()).pathname, "/app/app-manager", "login lost deep link");
  await setMode("desktop");
  const appWindow = page.getByRole("region", { name: "App manager", exact: true });
  const before = await appWindow.boundingBox();
  const title = appWindow.locator(".shell-window__titlebar");
  const titleBox = await title.boundingBox();
  await page.mouse.move(titleBox.x + 150, titleBox.y + 20);
  await page.mouse.down();
  await page.mouse.move(titleBox.x + 220, titleBox.y + 40, { steps: 5 });
  await page.mouse.up();
  assert((await appWindow.boundingBox()).x > before.x + 40, `window did not move: ${JSON.stringify(before)} -> ${JSON.stringify(await appWindow.boundingBox())}`);
  const resize = appWindow.getByRole("button", { name: "Resize App manager with arrow keys" });
  await resize.focus();
  const width = (await appWindow.boundingBox()).width;
  await page.keyboard.press("Shift+ArrowLeft");
  assert((await appWindow.boundingBox()).width < width, "keyboard resize failed");
  await appWindow.getByRole("button", { name: "Maximize App manager", exact: true }).click();
  assert((await appWindow.boundingBox()).width > width, "maximize failed");
  await appWindow.getByRole("button", { name: "Restore App manager", exact: true }).click();
  await page.getByRole("button", { name: "Open launcher", exact: true }).click();
  const menu = page.getByRole("dialog");
  await menu.getByRole("searchbox").fill("Settings");
  assert.equal(await menu.getByRole("link").count(), 1);
  await page.keyboard.press("Escape");
  await menu.waitFor({ state: "detached" });
  if (process.env.RUMAHL_SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.RUMAHL_SCREENSHOT_DIR, "desktop.png") });
  await setMode("launcher");
  assert.equal(await page.locator(".shell").getAttribute("data-shell-mode"), "launcher");
  assert.equal(await page.getByRole("button", { name: "Close App manager", exact: true }).count(), 0);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Settings" }).click();
  await page.getByRole("navigation", { name: "Settings", exact: true }).getByRole("link", { name: "Shell mode" }).click();
  assert.equal(new URL(page.url()).pathname, "/settings/display");
  await page.reload();
  await page.getByRole("heading", { name: "Shell mode" }).waitFor();
  await page.goBack();
  await page.getByRole("heading", { name: "Settings", exact: true, level: 1 }).waitFor();
  await page.goForward();
  await page.getByRole("heading", { name: "Shell mode" }).waitFor();
  await setMode("desktop");
  await page.getByRole("region", { name: "Settings", exact: true }).waitFor();
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Home", exact: true }).click();
  // Launch a real authorized installation, including an app-local deep route.
  const appCookies = [];
  page.on("request", (request) => {
    if (new URL(request.url()).hostname.endsWith(".apps.localhost")) appCookies.push(request.headers().cookie);
  });
  await setMode("launcher");
  await page.goto(`${info.origin}/apps`);
  await page.getByRole("link", { name: "Isolated test app", exact: true }).click();
  const iframe = page.locator('iframe[title="Isolated test app"]');
  await page.frameLocator('iframe[title="Isolated test app"]').getByText("Isolated app ready", { exact: true }).waitFor();
  const assetUrl = await iframe.getAttribute("src");
  assert.notEqual(new URL(assetUrl).hostname, new URL(info.origin).hostname);
  assert.equal(await iframe.getAttribute("sandbox"), "allow-scripts");
  await iframe.contentFrame().locator("body").evaluate((body) => { body.dataset.persistenceProbe = "preserved"; });
  await setMode("desktop");
  assert.equal(await iframe.contentFrame().locator("body").getAttribute("data-persistence-probe"), "preserved");
  await page.getByRole("button", { name: "Minimize Isolated test app", exact: true }).click();
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("button", { name: "Isolated test app", exact: true }).click();
  assert.equal(await iframe.contentFrame().locator("body").getAttribute("data-persistence-probe"), "preserved");
  await setMode("launcher");
  if (process.env.RUMAHL_SCREENSHOT_DIR) {
    await page.getByRole("button", { name: "Launcher home", exact: true }).click();
    await page.getByRole("heading", { name: "Your apps. Your space.", exact: true }).waitFor();
    await page.screenshot({ path: join(process.env.RUMAHL_SCREENSHOT_DIR, "launcher.png") });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: join(process.env.RUMAHL_SCREENSHOT_DIR, "launcher-mobile.png") });
    assert(await page.evaluate(() => globalThis.document.documentElement.scrollWidth <= globalThis.innerWidth), "mobile viewport overflows");
    await page.setViewportSize({ width: 1280, height: 720 });
  }
  await setMode("desktop");
  await page.goto(`${info.origin}/app/com.rumahl.host-test`);
  await page.getByRole("region", { name: "Isolated test app", exact: true }).waitFor();
  await page.frameLocator('iframe[title="Isolated test app"]').getByText("Isolated app ready", { exact: true }).waitFor();
  await setMode("launcher");
  await page.goto(`${info.origin}/app/com.rumahl.host-test/document/42?view=detail`);
  await page.frameLocator('iframe[title="Isolated test app"]').getByText("Isolated app ready", { exact: true }).waitFor();
  assert.equal(await page.frameLocator('iframe[title="Isolated test app"]').locator("body").getAttribute("data-route"), "#/document/42?view=detail");
  assert(appCookies.length >= 4);
  assert(appCookies.every((cookie) => cookie === undefined), "shell cookie sent to app origin");
  // Native HTTPS request routed by the app host must never reach shell APIs.
  const denied = await assetStatus(new URL("/api/v1/shell/snapshot", assetUrl), info.certificate);
  assert.equal(denied, 404);
  execFileSync(process.argv[3], ["revoke", join(state, "data")]);
  const oldAsset = await assetStatus(assetUrl, info.certificate);
  assert.equal(oldAsset, 404, "revoked lease still served app assets");
  await page.locator('iframe[title="Isolated test app"]').waitFor({ state: "detached", timeout: 35_000 });
  await setMode("launcher");
  await page.goto(`${info.origin}/apps`);
  await page.getByText("No apps are available for this account.", { exact: true }).waitFor();
  assert.equal(await page.locator('iframe[title="Isolated test app"]').count(), 0);
  await setMode("desktop");
  await page.goto(`${info.origin}/`);
  await page.locator(".shell").waitFor();
  // One account, two independent browser profiles, and a second tab of one profile.
  await page.goto(`${info.origin}/settings/display`);
  await page.getByRole("combobox", { name: "Save for" }).selectOption("device");
  await page.getByRole("button", { name: "Use account setting", exact: true }).click();
  await page.waitForFunction(() => !globalThis.document.querySelector('fieldset').disabled);
  const secondContext = await browser.newContext();
  try {
    const second = await secondContext.newPage();
    second.on("pageerror", (error) => errors.push(error.message));
    await login(second);
    await page.getByRole("combobox", { name: "Save for" }).selectOption("user");
    await page.getByRole("button", { name: "Launcher", exact: true }).click();
    await page.locator('.shell[data-shell-mode="launcher"]').waitFor();
    await second.locator('.shell[data-shell-mode="launcher"]').waitFor();
    await setMode("desktop", second);
    await synchronized(page, second);
    await page.getByRole("button", { name: "Desktop", exact: true }).click();
    await page.locator('.shell[data-shell-mode="desktop"]').waitFor();
    await page.getByRole("button", { name: "Launcher", exact: true }).click();
    await page.locator('.shell[data-shell-mode="launcher"]').waitFor();
    assert.equal(await second.locator(".shell").getAttribute("data-shell-mode"), "desktop");
    const sameProfileTab = await secondContext.newPage();
    await sameProfileTab.goto(`${info.origin}/`);
    await sameProfileTab.waitForFunction(() => !globalThis.document.querySelector('select[aria-label="Shell mode"]').disabled);
    assert.equal(await sameProfileTab.locator(".shell").getAttribute("data-shell-mode"), "desktop");
    await synchronized(second, page);
    await setMode("launcher", second);
    await sameProfileTab.locator('.shell[data-shell-mode="launcher"]').waitFor();
    await second.reload();
    await second.locator('.shell[data-shell-mode="launcher"]').waitFor();
    await synchronized(page, second);
    assert.equal(new URL(second.url()).searchParams.has("mode"), false);
    assert(await second.evaluate(() => Object.keys(globalThis.localStorage).some((key) => key.startsWith("rumahl.preferences.v1:"))), "preferences not persisted locally");
  } finally { await secondContext.close(); }
  await setMode("desktop");
  await page.goto(`${info.origin}/`);
  // Save a snapped workspace and app folder; verify a real browser reload.
  await page.goto(`${info.origin}/app/files`);
  await page.getByRole("heading", { name: "Files", exact: true, level: 1 }).waitFor();
  await page.getByRole("button", { name: "Snap Files left", exact: true }).click();
  await page.waitForFunction(() => Math.abs(globalThis.document.querySelector('[data-window-id="app:files"]').getBoundingClientRect().width - globalThis.document.querySelector(".window-layer").getBoundingClientRect().width / 2) < 2);
  const snapped = await page.getByRole("region", { name: "Files", exact: true }).boundingBox();
  const workArea = await page.locator(".window-layer").boundingBox();
  assert(Math.abs(snapped.width - workArea.width / 2) < 2);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Settings", exact: true }).click();
  await page.getByRole("navigation", { name: "Settings", exact: true }).getByRole("link", { name: "Workspace", exact: true }).click();
  await page.getByRole("button", { name: "Save arrangement", exact: true }).click();
  await page.getByRole("button", { name: "Save arrangement", exact: true }).waitFor();
  await page.getByLabel("Folder name", { exact: true }).fill("Tools");
  await page.getByRole("checkbox", { name: "Files", exact: true }).check();
  const folderSaved = page.waitForResponse(r => r.url().includes("/api/v1/shell/workspace?") && r.request().method() === "PUT");
  await page.getByRole("button", { name: "Save folder", exact: true }).click();
  assert.equal((await folderSaved).status(), 200);
  await page.reload();
  await page.getByRole("region", { name: "Files", exact: true }).waitFor();
  assert(Math.abs((await page.getByRole("region", { name: "Files", exact: true }).boundingBox()).width - snapped.width) < 2);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("button", { name: "Files", exact: true }).click();
  const files = page.getByRole("region", { name: "Files", exact: true });
  await files.getByLabel("New folder", { exact: true }).fill("Documents");
  await files.getByRole("button", { name: "Save", exact: true }).click();
  await files.getByRole("button", { name: "Documents/", exact: true }).click();
  await files.getByLabel("Upload file").setInputFiles({ name: "hello.txt", mimeType: "text/plain", buffer: Buffer.from("workspace upload") });
  await files.getByRole("link", { name: "hello.txt", exact: true }).waitFor();
  const downloadUrl = await files.getByRole("link", { name: "hello.txt", exact: true }).getAttribute("href");
  const downloaded = await page.evaluate(async path => { const response = await globalThis.fetch(path); return { text: await response.text(), disposition: response.headers.get("content-disposition") }; }, downloadUrl);
  assert.equal(downloaded.text, "workspace upload");
  assert(downloaded.disposition.startsWith("attachment;"));
  await files.getByRole("button", { name: "Rename", exact: true }).click();
  await files.getByLabel("Rename", { exact: true }).fill("renamed.txt");
  await files.getByRole("button", { name: "Save", exact: true }).click();
  await files.getByRole("link", { name: "renamed.txt", exact: true }).waitFor();
  await files.getByRole("button", { name: "Move", exact: true }).click();
  await files.getByRole("button", { name: "My files", exact: true }).click();
  await files.getByRole("button", { name: "Move here", exact: true }).click();
  await files.getByRole("link", { name: "renamed.txt", exact: true }).waitFor();
  const row = files.locator("li").filter({ has: page.getByRole("link", { name: "renamed.txt", exact: true }) });
  await row.getByRole("button", { name: "Delete", exact: true }).click();
  await files.getByRole("alertdialog").getByRole("button", { name: "Delete", exact: true }).click();
  await files.getByRole("link", { name: "renamed.txt", exact: true }).waitFor({ state: "detached" });
  // Saved account folders are delivered to an independent browser profile.
  const workspaceContext = await browser.newContext();
  const workspacePage = await workspaceContext.newPage();
  await login(workspacePage);
  await workspacePage.getByRole("button", { name: "Launcher home", exact: true }).click();
  await workspacePage.getByRole("button", { name: "Tools", exact: true }).click();
  await workspacePage.getByRole("link", { name: "Files", exact: true }).waitFor();
  await workspaceContext.close();
  await page.getByRole("button", { name: "Search system", exact: true }).click();
  await page.locator(".command-palette").waitFor();
  await page.evaluate(() => { globalThis.__rumahlRefreshProbe = "preserved"; });

  const app = join(frontend, "packages/shell/src/shell/ShellLayout.tsx");
  const source = await readFile(app, "utf8");
  assert(source.includes('data-dev-probe="changed"'), "expected disposable test fixture");
  await writeFile(app, source.replace('data-dev-probe="changed"', 'data-dev-probe="browser-hot"'));
  await page.locator('[data-dev-probe="browser-hot"]').waitFor();
  assert.equal(await page.evaluate(() => globalThis.__rumahlRefreshProbe), "preserved");
  assert.equal(await page.locator(".command-palette").count(), 1, "React state lost during refresh");

  await appendFile(join(frontend, "packages/shell/src/styles.css"), "\n.shell { outline: 3px solid rgb(1, 2, 3); }\n");
  await page.waitForFunction(() => globalThis.getComputedStyle(globalThis.document.querySelector(".shell")).outlineColor === "rgb(1, 2, 3)");
  assert.deepEqual(errors, [], "browser console/hydration errors");
  console.log("PASS: Chromium deep-link login, SSR/hydration, nested routes, modes, app isolation/revocation, user/device settings sync and React/CSS hot reload");
} finally {
  await browser.close();
}
