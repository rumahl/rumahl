import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath, URL } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const temporaryDirectory = await mkdtemp(join(tmpdir(), "rumahl-ssr-"));
const socketPath = join(temporaryDirectory, "render.sock");
const child = spawn(process.execPath, ["server.js"], {
  cwd: packageRoot,
  env: { ...process.env, RUMAHL_SSR_SOCKET: socketPath },
  stdio: ["ignore", "ignore", "pipe"]
});
let errorOutput = "";
child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => { errorOutput += chunk; });

try {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (child.exitCode !== null) throw new Error(errorOutput || "SSR server exited early");
    try {
      assert.equal((await stat(socketPath)).mode & 0o777, 0o600);
      break;
    } catch (error) {
      if (attempt === 99) throw error;
      await delay(25);
    }
  }

  const fixture = JSON.parse(await readFile(new URL("../../../../ui-contracts/fixtures/snapshots/authenticated.json", import.meta.url)));
  const build = JSON.parse(await readFile(new URL("../../shell/dist/build-id.json", import.meta.url)));
  fixture.shellBuildId = build.shellBuildId;
  const payload = JSON.stringify({
    nonce: "abcdefghijklmnop",
    snapshot: fixture,
    frameOrigins: ["https://weather.apps.rumahl.com"]
  });
  const result = await post(payload);
  assert.equal(result.status, 200);
  assert.match(result.headers["cache-control"], /no-store/);
  assert.match(result.headers["content-security-policy"], /frame-src 'self' https:\/\/weather\.apps\.rumahl\.com/);
  assert.match(result.body, /data-shell-ssr="1"/);
  assert.match(result.body, /Ada Example/);
  assert.match(result.body, /id="rumahl-shell-snapshot"/);
  assert.equal((await post(payload, { Cookie: "session=not-allowed" })).status, 400);
  assert.equal((await post(JSON.stringify({
    nonce: "abcdefghijklmnop",
    snapshot: fixture,
    frameOrigins: ["http://unsafe.example"]
  }))).status, 400);
  assert.equal((await post("x".repeat(301 * 1024))).status, 413);
} finally {
  if (child.exitCode === null && child.signalCode === null) {
    child.kill();
    await new Promise((resolve) => child.once("exit", resolve));
  }
  await rm(temporaryDirectory, { recursive: true, force: true });
}

function post(body, extraHeaders = {}) {
  return new Promise((resolve, reject) => {
    const outgoing = request({
      socketPath,
      method: "POST",
      path: "/render",
      headers: { "Content-Type": "application/json", ...extraHeaders }
    }, (incoming) => {
      let responseBody = "";
      incoming.setEncoding("utf8");
      incoming.on("data", (chunk) => { responseBody += chunk; });
      incoming.on("end", () => resolve({ status: incoming.statusCode, headers: incoming.headers, body: responseBody }));
    });
    outgoing.on("error", reject);
    outgoing.end(body);
  });
}
