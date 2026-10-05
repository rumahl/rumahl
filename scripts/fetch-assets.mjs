import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const lockPath = join(root, "assets", "upstream.lock.json");

const log = (message) => console.log(`[assets:fetch] ${message}`);
const warn = (message) => console.warn(`[assets:fetch] ${message}`);

const offline = process.env.RUMAHL_ASSETS_OFFLINE === "1";
const strict = process.env.RUMAHL_ASSETS_STRICT === "1";
const force = process.env.RUMAHL_ASSETS_FORCE === "1";

if (!existsSync(lockPath)) {
  warn("no assets/upstream.lock.json; skipping upstream asset fetch");
  process.exit(0);
}

const lock = JSON.parse(readFileSync(lockPath, "utf8"));
const target = isAbsolute(lock.extractTo) ? lock.extractTo : join(root, lock.extractTo);
const marker = join(target, ".pin");

if (!force && existsSync(marker) && readFileSync(marker, "utf8").trim() === lock.commit) {
  log(`upstream already at ${lock.commit.slice(0, 12)}; nothing to do`);
  process.exit(0);
}

if (offline) {
  warn("RUMAHL_ASSETS_OFFLINE=1 set; using existing assets/upstream (if any)");
  process.exit(0);
}

log(`downloading ${lock.repository} @ ${lock.commit.slice(0, 12)}`);

let archive;
try {
  const response = await fetch(lock.archive, { redirect: "follow" });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  archive = Buffer.from(await response.arrayBuffer());
} catch (error) {
  warn(`download failed: ${error.message}`);
  if (existsSync(marker)) warn("keeping previously fetched upstream assets");
  else warn("continuing without upstream assets");
  process.exit(strict ? 1 : 0);
}

const digest = createHash("sha256").update(archive).digest("hex");
if (digest !== lock.sha256) {
  console.error(`[assets:fetch] checksum mismatch for ${lock.archive}`);
  console.error(`[assets:fetch]   expected ${lock.sha256}`);
  console.error(`[assets:fetch]   actual   ${digest}`);
  console.error(
    "[assets:fetch] refusing to use the archive; update assets/upstream.lock.json if upstream legitimately changed"
  );
  process.exit(1);
}

const staging = mkdtempSync(join(tmpdir(), "rumahl-assets-"));
const archivePath = join(staging, "upstream.tar.gz");
const unpacked = join(staging, "unpacked");
mkdirSync(unpacked);
writeFileSync(archivePath, archive);

try {
  const strip = Number.isInteger(lock.stripComponents) ? lock.stripComponents : 1;
  execFileSync("tar", ["-xzf", archivePath, "-C", unpacked, "--strip-components", String(strip)], {
    stdio: "inherit"
  });
} catch (error) {
  rmSync(staging, { recursive: true, force: true });
  warn(`extraction failed: ${error.message}`);
  process.exit(strict ? 1 : 0);
}

rmSync(target, { recursive: true, force: true });
mkdirSync(dirname(target), { recursive: true });
cpSync(unpacked, target, { recursive: true });
writeFileSync(marker, `${lock.commit}\n`);
rmSync(staging, { recursive: true, force: true });

log(`upstream extracted to ${lock.extractTo}`);
