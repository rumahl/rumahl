import { readdir, readFile, writeFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { shellBuildId } from "./build-id.js";

const forbiddenDemoMarkers = ["demo-only-revision-001", "/src/demo/"];
const files = await collectFiles("dist");

for (const file of files) {
  if (![".html", ".js", ".json", ".map"].includes(extname(file))) continue;
  const contents = await readFile(file, "utf8");
  const marker = forbiddenDemoMarkers.find((candidate) => contents.includes(candidate));
  if (marker) {
    throw new Error(`production bundle contains demo marker ${marker} in ${file}`);
  }
}
await writeFile("dist/build-id.json", JSON.stringify({ shellBuildId: shellBuildId() }));

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
