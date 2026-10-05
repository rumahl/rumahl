import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
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
    mkdirSync(target, { recursive: true });
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
