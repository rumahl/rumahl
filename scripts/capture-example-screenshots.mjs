#!/usr/bin/env node
// Captures optional store artwork for the example apps. Never required for a
// build: the store falls back to generated monogram tiles without screenshots.
import { existsSync, mkdirSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { fileURLToPath } from "node:url";

const repo = resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = process.argv.slice(2);
const value = (name) => { const index = args.indexOf(name); return index >= 0 ? args[index + 1] : undefined; };
const apps = value("--apps");
const out = value("--out");
if (!apps || !out) {
  console.error("usage: capture-example-screenshots.mjs --apps APPS_DIR --out MEDIA_DIR");
  process.exit(2);
}
if (!existsSync(join(repo, "frontend", "node_modules", "playwright"))) {
  console.error("playwright is not installed; skipping screenshots");
  process.exit(2);
}

const require = createRequire(join(repo, "frontend", "package.json"));
let chromium;
try {
  ({ chromium } = require("playwright"));
} catch {
  console.error("playwright could not be loaded; skipping screenshots");
  process.exit(2);
}

const target = resolve(out);
mkdirSync(target, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  for (const slug of readdirSync(apps).sort()) {
    const index = join(apps, slug, "frontend", "index.html");
    if (!existsSync(index)) continue;
    const url = pathToFileURL(index).href;
    await page.setViewportSize({ width: 960, height: 600 });
    await page.goto(url, { waitUntil: "networkidle" });
    await page.screenshot({ path: join(target, `${slug}-screen.png`) });
    await page.setViewportSize({ width: 512, height: 512 });
    await page.goto(url, { waitUntil: "networkidle" });
    await page.screenshot({ path: join(target, `${slug}-icon.png`) });
    console.log(`captured ${slug}`);
  }
} finally {
  await browser.close();
}
