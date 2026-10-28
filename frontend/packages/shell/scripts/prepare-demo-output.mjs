/* global process, console */
import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const target = process.argv[2];
if (target !== "demo" && target !== "demo-widget") {
  console.error("usage: prepare-demo-output.mjs <demo|demo-widget>");
  process.exit(1);
}

const directory = target === "demo" ? "dist-demo" : "dist-demo-widget";
const entry = target === "demo" ? "demo.html" : "demo-widget.html";
const source = join(directory, entry);
const destination = join(directory, "index.html");

if (existsSync(source)) renameSync(source, destination);
if (!existsSync(destination)) {
  console.error(`missing ${destination}; did the Vite build run?`);
  process.exit(1);
}
if (target === "demo") {
  // The public demo is static, so it ships its own small cache-only worker.
  const workerSource = readFileSync("demo-service-worker.js", "utf8");
  writeFileSync(join(directory, "service-worker.js"), workerSource.replace("__RUMAHL_DEMO_SW_VERSION__", `demo-${Date.now()}`));
  console.log("[demo] service-worker.js ready");
}
console.log(`[demo] ${destination} ready`);
