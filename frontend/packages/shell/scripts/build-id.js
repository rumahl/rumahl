import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const sourceRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "src");

export function shellBuildId() {
  const hash = createHash("sha256");
  for (const path of sourceFiles(sourceRoot)) {
    hash.update(path.slice(sourceRoot.length));
    hash.update(readFileSync(path));
  }
  return `shell-${hash.digest("hex").slice(0, 24)}`;
}

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.name !== "demo" && !entry.name.endsWith(".test.tsx") && !entry.name.endsWith(".test.ts"))
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? sourceFiles(path) : [path];
    })
    .sort();
}
