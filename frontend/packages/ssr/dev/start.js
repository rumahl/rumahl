#!/usr/bin/env node
import console from "node:console";
import { setTimeout, clearTimeout, setInterval, clearInterval } from "node:timers";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { watch } from "node:fs";
import { chmod, lstat, mkdir, mkdtemp, open, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import { startGateway } from "./gateway.js";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const { values } = parseArgs({ options: {
  "state-dir": { type: "string", default: join(repo, ".rumahl-dev") },
  port: { type: "string", default: "8443" },
  binary: { type: "string" },
  cert: { type: "string" }, key: { type: "string" },
  poll: { type: "boolean", default: false },
  "no-watch": { type: "boolean", default: false },
  help: { type: "boolean", default: false }
} });
if (values.help) {
  console.log(`rumahl developer server
Usage: pnpm --dir frontend dev [options]
  --port PORT       HTTPS loopback port (default 8443; 0 selects a free port)
  --state-dir PATH  Separate persistent development state (default .rumahl-dev)
  --binary PATH     Use a compatible prebuilt platform binary; no Cargo required
  --cert PATH       Existing localhost TLS certificate (with --key PATH)
  --key PATH        Matching TLS private key
  --poll            Poll files for shared folders/WSL
  --no-watch        Disable Rust rebuilds (frontend HMR stays active)

Only localhost is exposed. Production services and data are never used.
See docs/development.md for certificate trust and rumahl OS operation.`);
  process.exit(0);
}
const state = resolve(values["state-dir"]);
const port = Number(values.port);
const children = new Set();
const watchers = [];
let platform;
let edge;
let runtime;
let ownsLock = false;
let stopping = false;
let timer;
let polling;
let env;
let binary;
let metadata;

async function command(executable, args, { capture = false, input, environment = process.env } = {}) {
  const child = spawn(executable, args, {
    cwd: repo, env: environment,
    stdio: [input === undefined ? "ignore" : "pipe", capture ? "pipe" : "inherit", "inherit"]
  });
  children.add(child);
  let output = "";
  child.stdout?.setEncoding("utf8");
  child.stdout?.on("data", (part) => { output += part; });
  if (input !== undefined) {
    child.stdin.on("error", () => {});
    child.stdin.end(input);
  }
  try {
    const code = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code) => resolve(code));
    });
    if (code !== 0) throw new Error(`${executable} failed (${code ?? "signal"})`);
    return output;
  } finally { children.delete(child); }
}
async function stop(child) {
  if (!child || !child.pid || child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise((resolve) => child.once("exit", resolve));
  child.kill("SIGTERM");
  const kill = setTimeout(() => child.kill("SIGKILL"), 5_000);
  try { await exited; } finally { clearTimeout(kill); children.delete(child); }
}
async function cleanup() {
  if (stopping) return;
  stopping = true;
  clearTimeout(timer);
  clearInterval(polling);
  for (const watcher of watchers) watcher.close();
  await Promise.all([...children].map(stop));
  await edge?.close();
  if (runtime) await rm(runtime, { recursive: true, force: true });
  if (ownsLock) await rm(join(state, "runner.lock"), { force: true });
}
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => { void cleanup().then(() => process.exit(0)); });
}

async function prepareState() {
  await mkdir(state, { recursive: true, mode: 0o700 });
  const info = await lstat(state);
  if (!info.isDirectory() || info.isSymbolicLink() || (info.mode & 0o077) !== 0 || info.uid !== process.getuid()) {
    throw new Error("Development state must be an owned directory with mode 0700");
  }
  const marker = join(state, "development-state.json");
  try {
    if (JSON.parse(await readFile(marker, "utf8")).kind !== "rumahl-development-v1") throw new Error("Unrecognized development state");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    if ((await readdir(state)).length) throw new Error("Refusing to use a nonempty directory without a development marker", { cause: error });
    await writeFile(marker, JSON.stringify({ kind: "rumahl-development-v1" }), { mode: 0o600, flag: "wx" });
  }
  const lock = join(state, "runner.lock");
  try {
    const pid = Number(await readFile(lock, "utf8"));
    if (!Number.isSafeInteger(pid) || pid < 1) throw new Error("Invalid developer-server lock; inspect runner.lock manually");
    try { process.kill(pid, 0); }
    catch (error) {
      if (error.code !== "ESRCH") throw error;
      await rm(lock);
    }
  } catch (error) { if (error.code !== "ENOENT") throw error; }
  const handle = await open(lock, "wx", 0o600);
  ownsLock = true;
  await handle.writeFile(String(process.pid));
  await handle.close();
  await mkdir(join(state, "data"), { mode: 0o700, recursive: true });
  runtime = await mkdtemp(join(tmpdir(), "rumahl-dev-"));
  await chmod(runtime, 0o700);
}
async function tls() {
  if (Boolean(values.cert) !== Boolean(values.key)) throw new Error("Supply both --cert and --key");
  if (values.cert) return { cert: resolve(values.cert), key: resolve(values.key) };
  const directory = join(state, "tls");
  await mkdir(directory, { mode: 0o700, recursive: true });
  const cert = join(directory, "localhost.pem");
  const key = join(directory, "localhost-key.pem");
  try { await stat(cert); await stat(key); }
  catch (error) {
    if (error.code !== "ENOENT") throw error;
    await command("openssl", ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "30",
      "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:::1",
      "-keyout", key, "-out", cert], { capture: true });
    await chmod(key, 0o600);
  }
  return { cert, key };
}
async function build() {
  await command("cargo", ["build", "--locked", "-p", "rumahl-platform-service"]);
}
async function launchPlatform() {
  await stop(platform);
  await rm(join(runtime, "gateway.sock"), { force: true });
  const child = spawn(binary, ["serve"], { cwd: repo, env, stdio: ["ignore", "inherit", "inherit"] });
  platform = child;
  children.add(child);
  let failed;
  child.once("error", (error) => { failed = error; });
  child.once("exit", () => { children.delete(child); });
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (failed) throw failed;
    if (child.exitCode !== null || child.signalCode !== null) throw new Error("Platform exited during startup");
    try { await stat(join(runtime, "gateway.sock")); return; }
    catch (error) { if (error.code !== "ENOENT") throw error; }
    await delay(50);
  }
  throw new Error("Platform socket did not become ready");
}
async function provision() {
  const ready = join(state, "account-ready");
  try { await stat(ready); return; } catch (error) { if (error.code !== "ENOENT") throw error; }
  const file = join(state, "credentials.json");
  let credentials;
  try { credentials = JSON.parse(await readFile(file, "utf8")); }
  catch (error) {
    if (error.code !== "ENOENT") throw error;
    credentials = { username: "developer", password: randomBytes(32).toString("base64url") };
    await writeFile(file, JSON.stringify(credentials, null, 2) + "\n", { mode: 0o600, flag: "wx" });
  }
  await command(binary, ["provision-account", credentials.username, "Developer", "--password-stdin"],
    { input: credentials.password, environment: env, capture: true });
  await writeFile(ready, "1\n", { mode: 0o600 });
}
function watchRust() {
  if (values.binary || values["no-watch"]) return;
  const roots = metadata.packages.filter((pkg) => metadata.workspace_members.includes(pkg.id)).map((pkg) => dirname(pkg.manifest_path));
  let building = false;
  let dirty = false;
  const rebuild = async () => {
    if (building || stopping) return;
    building = true;
    while (dirty && !stopping) {
      dirty = false;
      console.log("[rust] Building changes; the running platform remains available.");
      try {
        await build();
        if (stopping) break;
        await launchPlatform();
        edge.reload();
        console.log("[rust] Platform restarted; browser reloading.");
      } catch (error) {
        if (!stopping) console.error(`[rust] ${error.message}. Fix and save to retry.`);
      }
    }
    building = false;
  };
  const changed = () => {
    dirty = true;
    clearTimeout(timer);
    timer = setTimeout(() => { void rebuild(); }, 250);
  };
  const relevant = (name) => name.endsWith(".rs") || name.endsWith("Cargo.toml") || name.endsWith("Cargo.lock");
  if (values.poll) {
    let previous;
    let scanning = false;
    const snapshot = async (directory) => {
      const files = [];
      for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (["target", ".git", "node_modules"].includes(entry.name)) continue;
        const path = join(directory, entry.name);
        if (entry.isDirectory()) files.push(...await snapshot(path));
        else if (entry.isFile() && relevant(path)) {
          const info = await stat(path);
          files.push(`${path}:${info.mtimeMs}:${info.size}`);
        }
      }
      return files;
    };
    polling = setInterval(async () => {
      if (scanning || stopping) return;
      scanning = true;
      try {
        const files = (await Promise.all(roots.map(snapshot))).flat();
        for (const name of ["Cargo.toml", "Cargo.lock"]) {
          const info = await stat(join(repo, name)); files.push(`${name}:${info.mtimeMs}:${info.size}`);
        }
        const next = files.sort().join("\n");
        if (previous !== undefined && next !== previous) changed();
        previous = next;
      } catch (error) { console.error(`[watch] ${error.message}`); }
      finally { scanning = false; }
    }, 1000);
  } else {
    for (const path of roots) watchers.push(watch(path, { recursive: true }, (_, name) => { if (name && relevant(String(name))) changed(); }));
    watchers.push(watch(repo, (_, name) => { if (["Cargo.toml", "Cargo.lock"].includes(String(name))) changed(); }));
  }
}

try {
  if (process.platform !== "linux") throw new Error("Use Linux or WSL2 for the Unix/Linux platform service");
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error("Invalid --port");
  await prepareState();
  const { cert, key } = await tls();
  if (values.binary) binary = resolve(values.binary);
  else {
    metadata = JSON.parse(await command("cargo", ["metadata", "--no-deps", "--format-version=1", "--locked"], { capture: true }));
    binary = join(metadata.target_directory, "debug/rumahl-platform-service");
    await build();
  }
  const buildId = `shell-dev-${randomUUID()}`;
  const buildFile = join(runtime, "build-id.json");
  await writeFile(buildFile, JSON.stringify({ shellBuildId: buildId }), { mode: 0o600 });
  const blocklist = join(state, "password-blocklist.txt");
  await writeFile(blocklist, "development-password-must-not-be-used\n", { mode: 0o600 });
  edge = await startGateway({ repo, runtime, state, cert, key, port, buildId, polling: values.poll });
  env = { ...process.env, RUMAHL_STATE_DIR: join(state, "data"), RUMAHL_PUBLIC_ORIGIN: edge.origin,
    RUMAHL_CLIENT_BUILD: buildFile, RUMAHL_PASSWORD_BLOCKLIST: blocklist,
    RUMAHL_GATEWAY_SOCKET: join(runtime, "gateway.sock"), RUMAHL_GATEWAY_SOCKET_ACCESS: "owner",
    RUMAHL_SSR_SOCKET: join(runtime, "ssr.sock"), RUMAHL_SSR_SOCKET_ACCESS: "owner" };
  await provision();
  await launchPlatform();
  watchRust();
  await writeFile(join(state, "server.json"), JSON.stringify({ origin: edge.origin, certificate: cert, buildId }), { mode: 0o600 });
  console.log(`\nrumahl developer server: ${edge.origin}\nLogin: developer\nPassword: ${join(state, "credentials.json")}\nTLS trust certificate: ${cert}\nReact/CSS: hot reload. Rust: ${values.binary || values["no-watch"] ? "manual binary restart" : "automatic rebuild/restart"}.\nCtrl+C stops the server; accounts and sessions are preserved.\n`);
} catch (error) {
  console.error(`Developer server: ${error.message}`);
  await cleanup();
  process.exitCode = 1;
}
