/**
 * Long-label and localization edge-case checks on the real shell.
 *
 * Injects long German-style compounds into representative labels and checks
 * that the page does not grow horizontally and that each label clips, ellipsizes
 * or wraps intentionally instead of escaping its container.
 *
 * Usage:
 *   pnpm --dir frontend long-text
 */
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const base = (process.env.RUMAHL_AUDIT_URL ?? "https://localhost:8443").replace(/\/$/, "");
const credentialsPath = process.env.RUMAHL_AUDIT_CREDENTIALS ?? resolve(here, "../../.rumahl-dev/credentials.json");
const insecure = process.env.RUMAHL_AUDIT_INSECURE !== "0";
const executablePath = process.env.RUMAHL_AUDIT_CHROME ?? "/usr/bin/google-chrome";
const failures = [];
const long = "Datenbankverschlüsselungsverwaltungsdienstleistung";

const credentials = (() => {
  if (process.env.RUMAHL_AUDIT_USER && process.env.RUMAHL_AUDIT_PASSWORD) {
    return { username: process.env.RUMAHL_AUDIT_USER, password: process.env.RUMAHL_AUDIT_PASSWORD };
  }
  const parsed = JSON.parse(readFileSync(credentialsPath, "utf8"));
  if (typeof parsed.username === "string" && typeof parsed.password === "string") return parsed;
  throw new Error(`No long-text credentials at ${credentialsPath}.`);
})();

function check(name, condition, detail) {
  if (!condition) failures.push({ name, detail });
}

const browser = await chromium.launch({
  headless: true,
  ...(existsSync(executablePath) ? { executablePath } : {}),
  args: insecure ? ["--ignore-certificate-errors"] : []
});

try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    colorScheme: "dark",
    ignoreHTTPSErrors: insecure
  });
  const page = await context.newPage();
  await page.goto(`${base}/login?next=%2F&user=${encodeURIComponent(credentials.username)}`, {
    waitUntil: "domcontentloaded",
    timeout: 60_000
  });
  await page.fill('input[name="password"]', credentials.password);
  await Promise.all([
    page.waitForNavigation({ waitUntil: "domcontentloaded" }).catch(() => {}),
    page.click('button[type="submit"]')
  ]);
  await page.waitForTimeout(2400);

  const probe = async (label, selector, optional = false) => {
    const result = await page.evaluate(({ selector, long }) => {
      const element = document.querySelector(selector);
      if (!(element instanceof HTMLElement)) return null;
      element.textContent = long;
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      const parent = element.parentElement?.getBoundingClientRect();
      return {
        rectRight: rect.right,
        parentRight: parent?.right ?? window.innerWidth,
        scrollWidth: element.scrollWidth,
        clientWidth: element.clientWidth,
        overflow: style.overflow,
        textOverflow: style.textOverflow,
        whiteSpace: style.whiteSpace,
        lineHeight: Number.parseFloat(style.lineHeight) || 0,
        height: rect.height,
        pageOverflow: document.documentElement.scrollWidth - window.innerWidth
      };
    }, { selector, long });
    if (result === null && optional) return;
    check(`${label}-present`, result !== null, selector);
    if (!result) return;
    check(`${label}-page-overflow`, result.pageOverflow <= 1, result);
    check(`${label}-parent-overflow`, result.rectRight <= result.parentRight + 2 || result.overflow === "hidden" || result.textOverflow === "ellipsis", result);
    check(
      `${label}-clip-or-wrap`,
      result.scrollWidth <= result.clientWidth + 2 ||
        result.overflow === "hidden" ||
        result.textOverflow === "ellipsis" ||
        ["normal", "break-word"].includes(result.whiteSpace) ||
        result.height <= result.lineHeight * 2 + 2,
      result
    );
    await page.screenshot({ path: resolve(here, "../.shots/phase2/long-text-" + label + ".png") });
  };

  await page.goto(`${base}/settings`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1600);
  await probe("settings-nav", ".settings-nav__text strong");
  await probe("settings-section", ".settings-content .section-title");
  await page.goto(`${base}/settings/display/appearance`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1200);
  await probe("settings-row", ".settings-content .setting .copy strong");

  await page.goto(`${base}/app/files`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1600);
  await probe("files-nav", ".files-nav__label");
  await probe("files-tile", ".files-tile__name");
  const listView = page.locator(".files-view-switch__icon--list");
  if (await listView.count()) {
    await listView.click({ force: true });
    await page.waitForTimeout(300);
  }
  await probe("files-row", ".files-row__label");

  await page.goto(`${base}/app/store`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1800);
  await probe("store-heading", ".store-page-head h1");
  await probe("store-card", ".store-grid-card__title strong", true);

  await page.goto(`${base}/`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1400);
  await probe("desktop-label", ".desktop-icon__label");

  await context.close();
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(`\nLong-text check failed: ${failures.length} issue(s)\n`);
  for (const failure of failures) console.error(`[${failure.name}] ${JSON.stringify(failure.detail)}`);
  process.exitCode = 1;
} else {
  console.log("Long-text check passed.");
}
