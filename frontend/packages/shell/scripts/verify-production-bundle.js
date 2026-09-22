import { readdir, readFile } from "node:fs/promises";
import { extname, join } from "node:path";

const forbiddenDemoMarkers = ["demo-only-revision-001", "shell-build-demo-only", "/src/demo/"];
const files = await collectFiles("dist");

for (const file of files) {
  if (![".html", ".js", ".json", ".map"].includes(extname(file))) continue;
  const contents = await readFile(file, "utf8");
  const marker = forbiddenDemoMarkers.find((candidate) => contents.includes(candidate));
  if (marker) {
    throw new Error(`production bundle contains demo marker ${marker} in ${file}`);
  }
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? collectFiles(path) : [path];
    })
  );
  return nested.flat();
}
