/**
 * Design-system acceptance audit for the running rumahl OS development shell.
 *
 * This is the enforcement layer for docs/design/design-system-contracts.md:
 * it checks the rendered result, not only tokens. It intentionally runs against
 * the real development server because the shell depends on authenticated state
 * and the desktop wallpaper.
 *
 * Usage:
 *   pnpm --dir frontend design-system:audit
 *
 * Override with:
 *   RUMAHL_AUDIT_URL, RUMAHL_AUDIT_USER, RUMAHL_AUDIT_PASSWORD,
 *   RUMAHL_AUDIT_CREDENTIALS, RUMAHL_AUDIT_INSECURE=0
 */
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { chromium } from "playwright";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const base = (process.env.RUMAHL_AUDIT_URL ?? "https://localhost:8443").replace(/\/$/, "");
const credentialsPath = process.env.RUMAHL_AUDIT_CREDENTIALS ?? resolve(here, "../../.rumahl-dev/credentials.json");
const insecure = process.env.RUMAHL_AUDIT_INSECURE !== "0";
const axeSource = readFileSync(require.resolve("axe-core/axe.min.js"), "utf8");

const credentials = (() => {
  if (process.env.RUMAHL_AUDIT_USER && process.env.RUMAHL_AUDIT_PASSWORD) {
    return { username: process.env.RUMAHL_AUDIT_USER, password: process.env.RUMAHL_AUDIT_PASSWORD };
  }
  try {
    const parsed = JSON.parse(readFileSync(credentialsPath, "utf8"));
    if (typeof parsed.username === "string" && typeof parsed.password === "string") return parsed;
  } catch {
    // The report below explains how to provide credentials.
  }
  throw new Error(
    `No audit credentials. Set RUMAHL_AUDIT_USER/RUMAHL_AUDIT_PASSWORD or provide ${credentialsPath}.`
  );
})();

const failures = [];
const notes = [];
const fail = (page, rule, detail) => failures.push({ page, rule, detail });

function contrast([fr, fg, fb], [br, bg, bb]) {
  const channel = (value) => {
    const v = value / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const luminance = (r, g, b) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
  const a = luminance(fr, fg, fb);
  const b = luminance(br, bg, bb);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

function rgba(value) {
  const match = /^rgba?\(([^)]+)\)$/i.exec(value.trim());
  if (!match) return null;
  const parts = match[1].split(/[,\s/]+/).filter(Boolean).map(Number);
  if (parts.length < 3 || !parts.slice(0, 3).every(Number.isFinite)) return null;
  return [parts[0], parts[1], parts[2], parts.length > 3 ? parts[3] : 1];
}

function composite(foreground, background) {
  const [r, g, b, a] = foreground;
  return [
    Math.round(r * a + background[0] * (1 - a)),
    Math.round(g * a + background[1] * (1 - a)),
    Math.round(b * a + background[2] * (1 - a))
  ];
}

async function login(context) {
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
  await page.waitForTimeout(1600);
  return page;
}

async function inspect(page, name, mobile = false) {
  await page.waitForTimeout(1800);
  const result = await page.evaluate(({ mobile }) => {
    const visible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" &&
        style.display !== "none" && Number(style.opacity) > 0.01;
    };
    const textNodes = [...document.querySelectorAll("body *")]
      .filter((element) => visible(element) && [...element.childNodes].some(
        (node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim().length > 1
      ))
      .map((element) => ({
        text: (element.textContent || "").trim().replace(/\s+/g, " ").slice(0, 100),
        size: Number.parseFloat(getComputedStyle(element).fontSize),
        className: typeof element.className === "string" ? element.className.slice(0, 80) : ""
      }));
    const targets = [...document.querySelectorAll("button, a[href], input:not([type=hidden]), select, textarea")]
      .filter(visible)
      .filter((element) => element.getAttribute("role") !== "switch")
      .filter((element) => !["radio", "checkbox"].includes(element.getAttribute("type") || ""))
      .map((element) => {
        const rect = element.getBoundingClientRect();
        return {
          name: (element.getAttribute("aria-label") || element.textContent || "").trim().replace(/\s+/g, " ").slice(0, 70),
          tag: element.tagName.toLowerCase(),
          width: Math.round(rect.width),
          height: Math.round(rect.height)
        };
      });
    return {
      overflow: Math.max(
        document.documentElement.scrollWidth - window.innerWidth,
        document.body.scrollWidth - window.innerWidth
      ),
      tooSmallText: textNodes.filter((item) => item.size > 0 && item.size < 12),
      tooSmallTargets: mobile ? targets.filter((item) => item.width < 44 || item.height < 44) : [],
      invalidLinkAria: [...document.querySelectorAll('a[href][aria-pressed]')].map((element) => ({
        text: (element.textContent || "").trim().slice(0, 60),
        pressed: element.getAttribute("aria-pressed")
      })),
      resizer: (() => {
        const element = document.querySelector(".settings-resizer");
        return element ? getComputedStyle(element).display : null;
      })()
    };
  }, { mobile });

  if (result.overflow > 1) fail(name, "horizontal-overflow", `${result.overflow}px`);
  if (result.tooSmallText.length) fail(name, "text-floor", JSON.stringify(result.tooSmallText.slice(0, 8)));
  if (result.tooSmallTargets.length) fail(name, "target-size", JSON.stringify(result.tooSmallTargets.slice(0, 10)));
  if (result.invalidLinkAria.length) fail(name, "link-aria", JSON.stringify(result.invalidLinkAria));
  if (name.startsWith("mobile-settings") && result.resizer !== "none") {
    fail(name, "mobile-settings-resizer", `display=${result.resizer ?? "missing"}`);
  }
  return result;
}

async function axe(page, name) {
  await page.evaluate(axeSource);
  const violations = await page.evaluate(async () => {
    const result = await window.axe.run(document, {
      runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"] }
    });
    return result.violations
      .filter((item) => item.impact === "critical" || item.impact === "serious")
      .map((item) => ({ id: item.id, impact: item.impact, nodes: item.nodes.length }));
  });
  if (violations.length) fail(name, "axe", JSON.stringify(violations));
}

async function focusWalk(page, name) {
  await page.evaluate(() => document.activeElement?.blur?.());
  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press("Tab");
    await page.waitForTimeout(60);
    const state = await page.evaluate(() => {
      const element = document.activeElement;
      if (!(element instanceof HTMLElement)) return null;
      const style = getComputedStyle(element);
      return {
        name: (element.getAttribute("aria-label") || element.textContent || "").trim().slice(0, 60),
        outline: style.outlineStyle !== "none" && Number.parseFloat(style.outlineWidth) > 0,
        shadow: style.boxShadow !== "none"
      };
    });
    if (state && !state.outline && !state.shadow) fail(name, "focus-indicator", state.name);
  }
}

async function widgetContrast(page, name) {
  await page.waitForFunction(() => {
    const widgets = [...document.querySelectorAll(".desktop-widget")];
    return widgets.length > 0 && widgets.every((widget) => widget.style.getPropertyValue("--adaptive-ink"));
  }, undefined, { timeout: 6000 }).catch(() => {});
  await page.waitForTimeout(350);
  const targets = await page.evaluate(() => {
    const selectors = [
      ".desktop-widget__time",
      ".desktop-widget--status strong",
      ".desktop-widget__eyebrow",
      ".desktop-widget > span"
    ];
    return selectors.flatMap((selector) => [...document.querySelectorAll(selector)].map((element) => {
      const rect = element.getBoundingClientRect();
      return {
        selector,
        text: (element.textContent || "").trim().slice(0, 80),
        color: getComputedStyle(element).color,
        size: Number.parseFloat(getComputedStyle(element).fontSize),
        x: rect.x + rect.width / 2,
        y: rect.y + rect.height / 2
      };
    }));
  });
  if (!targets.length) {
    notes.push(`${name}: no desktop widgets visible`);
    return;
  }
  await page.evaluate(() => {
    document.querySelectorAll(".desktop-widget__time, .desktop-widget--status strong, .desktop-widget__eyebrow, .desktop-widget > span")
      .forEach((element) => { element.style.visibility = "hidden"; });
  });
  const screenshot = PNG.sync.read(await page.screenshot({ type: "png" }));
  await page.evaluate(() => {
    document.querySelectorAll(".desktop-widget__time, .desktop-widget--status strong, .desktop-widget__eyebrow, .desktop-widget > span")
      .forEach((element) => { element.style.visibility = ""; });
  });
  const scaleX = screenshot.width / Math.max(1, await page.evaluate(() => window.innerWidth));
  const scaleY = screenshot.height / Math.max(1, await page.evaluate(() => window.innerHeight));
  for (const target of targets) {
    const x = Math.max(0, Math.min(screenshot.width - 1, Math.round(target.x * scaleX)));
    const y = Math.max(0, Math.min(screenshot.height - 1, Math.round(target.y * scaleY)));
    const index = (y * screenshot.width + x) * 4;
    const background = [screenshot.data[index], screenshot.data[index + 1], screenshot.data[index + 2]];
    const ink = rgba(target.color);
    if (!ink) {
      fail(name, "widget-contrast", `unparseable colour ${target.color}`);
      continue;
    }
    const foreground = composite(ink, background);
    const ratio = contrast(foreground, background);
    const required = target.size >= 24 ? 3 : 4.5;
    if (ratio < required) fail(name, "widget-contrast", `${target.text}: ${ratio.toFixed(2)}:1 (need ${required}:1)`);
  }
}

const executablePath = process.env.RUMAHL_AUDIT_CHROME ?? "/usr/bin/google-chrome";
const browser = await chromium.launch({
  headless: true,
  ...(existsSync(executablePath) ? { executablePath } : {}),
  args: insecure ? ["--ignore-certificate-errors"] : []
});

try {
  const surfaces = [
    { name: "desktop-root", route: "/", mobile: false },
    { name: "settings", route: "/settings", mobile: false },
    { name: "files", route: "/app/files", mobile: false },
    { name: "store", route: "/app/store", mobile: false },
    { name: "mobile-root", route: "/", mobile: true },
    { name: "mobile-settings", route: "/settings/mobile", mobile: true },
    { name: "mobile-files", route: "/app/files", mobile: true },
    { name: "mobile-store", route: "/app/store", mobile: true }
  ];

  for (const surface of surfaces) {
    const context = await browser.newContext({
      viewport: surface.mobile ? { width: 390, height: 664 } : { width: 1440, height: 900 },
      ...(surface.mobile ? {
        isMobile: true,
        hasTouch: true,
        deviceScaleFactor: 2,
        userAgent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Mobile Safari/537.36"
      } : {}),
      reducedMotion: "reduce",
      ignoreHTTPSErrors: insecure
    });
    const page = await login(context);
    if (surface.mobile) {
      await page.evaluate(() => localStorage.setItem("rumahl.mobile.tour.v1", "done"));
      await page.reload({ waitUntil: "domcontentloaded" });
      await page.waitForTimeout(1400);
    }
    await page.goto(`${base}${surface.route}`, { waitUntil: "domcontentloaded" });
    if (surface.route.startsWith("/app/files")) {
      await page.waitForFunction(() => {
        const surface = document.querySelector(".files-surface");
        return surface?.getAttribute("aria-busy") === "false";
      }, undefined, { timeout: 10_000 }).catch(() => {});
      await page.waitForTimeout(250);
    }
    await inspect(page, surface.name, surface.mobile);
    await axe(page, surface.name);
    if (surface.name === "desktop-root") {
      await widgetContrast(page, surface.name);
      await focusWalk(page, surface.name);
    }
    await context.close();
  }
  notes.push("Geometry audited with reduced motion to avoid transient entrance transforms.");

} finally {
  await browser.close();
}

for (const note of notes) console.log(`note: ${note}`);
if (failures.length) {
  console.error(`\nDesign-system audit failed: ${failures.length} issue(s)\n`);
  for (const item of failures) console.error(`[${item.page}] ${item.rule}: ${item.detail}`);
  console.error("\nFix the contract violation or document a deliberate OS adaptation in DESIGN.md.");
  process.exitCode = 1;
} else {
  console.log("Design-system audit passed.");
}
