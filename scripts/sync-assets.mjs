import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// Source directory -> destination directory. Each rule copies the entries of
// `from` flat into `to`. Destinations are generated and gitignored, so edit
// the stores, never the generated directory.
//
// `assets/upstream` is a pinned checkout of https://github.com/rumahl/assets
// (multi-license; see scripts/fetch-assets.mjs). Rules whose source does not
// exist yet are skipped, so the upstream rules can be added before the artwork
// lands upstream.
const copies = [
  { from: "assets/upstream/wallpapers", to: "frontend/packages/shell/src/assets" }
];

const cleared = new Set();
let total = 0;

for (const { from, to } of copies) {
  const source = join(root, from);
  const target = join(root, to);

  if (!existsSync(source)) {
    console.log(`[assets:sync] skip ${from} (not present)`);
    continue;
  }

  if (!cleared.has(target)) {
    rmSync(target, { recursive: true, force: true });
    // Recreating a freshly removed directory can race with file watchers
    // observing the deletion (dev servers, editors): libuv's recursive mkdir
    // then sees a vanished ancestor and reports ENOENT. A short retry makes
    // the deterministic rebuild win; anything still failing is a real error.
    for (let attempt = 1; ; attempt += 1) {
      try {
        mkdirSync(target, { recursive: true });
        break;
      } catch (error) {
        if (error.code !== "ENOENT" || attempt === 10) throw error;
        await delay(50 * attempt);
      }
    }
    cleared.add(target);
  }

  const entries = readdirSync(source);
  for (const entry of entries) {
    cpSync(join(source, entry), join(target, entry), { recursive: true });
  }

  total += entries.length;
  console.log(`[assets:sync] ${from} -> ${to} (${entries.length})`);
}

console.log(`[assets:sync] ${total} entr${total === 1 ? "y" : "ies"} synced`);
