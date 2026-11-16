/**
 * Focused architecture checks for the Iris/Ground theme bases.
 *
 * Requires the local development shell and developer credentials. These are
 * intentionally narrower than design-system-audit.mjs: they check that theme
 * tokens reach rendered components and that Ground overrides do not require
 * `!important`.
 *
 * Usage:
 *   pnpm --dir frontend theme-architecture
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

const credentials = (() => {
  if (process.env.RUMAHL_AUDIT_USER && process.env.RUMAHL_AUDIT_PASSWORD) {
    return { username: process.env.RUMAHL_AUDIT_USER, password: process.env.RUMAHL_AUDIT_PASSWORD };
  }
  const parsed = JSON.parse(readFileSync(credentialsPath, "utf8"));
  if (typeof parsed.username === "string" && typeof parsed.password === "string") return parsed;
  throw new Error(`No architecture-test credentials at ${credentialsPath}.`);
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
  await page.waitForTimeout(1800);
  await page.goto(`${base}/app/files`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1400);

  const tokenToRender = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    const window = document.querySelector(".shell-window");
    return {
      token: root.getPropertyValue("--rumahl-ui-shape-radius-window").trim(),
      rendered: window ? getComputedStyle(window).borderRadius : null
    };
  });
  check(
    "token-to-render-window-radius",
    tokenToRender.rendered !== null && tokenToRender.token === tokenToRender.rendered,
    tokenToRender
  );

  const irisTokens = await page.evaluate(() => {
    const root = document.documentElement;
    const windowElement = document.querySelector(".shell-window");
    const dock = document.querySelector(".os-dock");
    if (!(windowElement instanceof HTMLElement) || !(dock instanceof HTMLElement)) return null;
    const previousTransition = windowElement.style.transition;
    windowElement.style.transition = "none";
    const read = () => ({
      windowRadius: getComputedStyle(windowElement).borderRadius,
      dockRadius: getComputedStyle(dock).borderRadius
    });
    const before = read();
    root.style.setProperty("--rumahl-ui-shape-radius-window", "5px");
    root.style.setProperty("--rumahl-ui-shape-radius-dock", "6px");
    const after = read();
    root.style.removeProperty("--rumahl-ui-shape-radius-window");
    root.style.removeProperty("--rumahl-ui-shape-radius-dock");
    windowElement.style.transition = previousTransition;
    return { before, after };
  });
  check("iris-window-token", irisTokens?.after.windowRadius === "5px", irisTokens);
  check("iris-dock-token", irisTokens?.after.dockRadius === "6px", irisTokens);

  await page.keyboard.press("Tab");
  const focus = await page.evaluate(() => {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement)) return null;
    const style = getComputedStyle(element);
    return {
      tag: element.tagName,
      className: typeof element.className === "string" ? element.className : "",
      outlineStyle: style.outlineStyle,
      outlineWidth: style.outlineWidth,
      outlineColor: style.outlineColor
    };
  });
  check(
    "keyboard-focus-visible",
    focus !== null &&
      focus.outlineStyle !== "none" &&
      Number.parseFloat(focus.outlineWidth) >= 2 &&
      focus.outlineColor !== "rgba(0, 0, 0, 0)",
    focus
  );

  const ground = await page.evaluate(() => {
    const root = document.documentElement;
    const shell = document.querySelector(".shell");
    const windowElement = document.querySelector(".shell-window");
    if (!(shell instanceof HTMLElement) || !(windowElement instanceof HTMLElement)) {
      return { error: "shell or window missing" };
    }
    root.dataset.themeBase = "ground";
    shell.dataset.themeBase = "ground";
    const probe = document.createElement("div");
    probe.className = "theme-architecture-probe";
    probe.style.width = "20px";
    probe.style.height = "20px";
    shell.append(probe);
    const menu = document.createElement("div");
    menu.className = "desktop-menu theme-architecture-menu";
    shell.append(menu);
    const previousTransition = windowElement.style.transition;
    windowElement.style.transition = "none";
    const read = () => ({
      probeRadius: getComputedStyle(probe).borderRadius,
      windowRadius: getComputedStyle(windowElement).borderRadius,
      menuBackground: getComputedStyle(menu).backgroundColor,
      menuRadius: getComputedStyle(menu).borderRadius
    });
    const before = read();
    root.style.setProperty("--ground-radius", "11px");
    const afterRadius = read();
    root.style.setProperty("--ground-radius", "0px");

    root.style.setProperty("--rumahl-ui-shape-radius-window", "7px");
    const afterTokenRadius = read();
    root.style.removeProperty("--rumahl-ui-shape-radius-window");

    shell.style.setProperty("--window-radius", "9px");
    const afterRecipeRadius = read();
    shell.style.removeProperty("--window-radius");

    shell.style.setProperty("--menu-background", "rgb(1, 2, 3)");
    shell.style.setProperty("--menu-radius", "4px");
    const afterMaterial = read();
    shell.style.removeProperty("--menu-background");
    shell.style.removeProperty("--menu-radius");

    const result = { before, afterRadius, afterTokenRadius, afterRecipeRadius, afterMaterial };
    probe.remove();
    menu.remove();
    windowElement.style.transition = previousTransition;
    root.style.removeProperty("--ground-radius");
    delete shell.dataset.themeBase;
    delete root.dataset.themeBase;
    return result;
  });
  check("ground-default-radius", ground.before?.probeRadius === "0px", ground);
  check("ground-radius-override", ground.afterRadius?.probeRadius === "11px", ground);
  check("ground-window-token", ground.afterTokenRadius?.windowRadius === "7px", ground);
  check("ground-window-recipe", ground.afterRecipeRadius?.windowRadius === "9px", ground);
  check("ground-material-override", ground.afterMaterial?.menuBackground === "rgb(1, 2, 3)", ground);
  check("ground-material-radius", ground.afterMaterial?.menuRadius === "4px", ground);

  await context.close();
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(`\nTheme architecture check failed: ${failures.length} issue(s)\n`);
  for (const failure of failures) console.error(`[${failure.name}] ${JSON.stringify(failure.detail)}`);
  process.exitCode = 1;
} else {
  console.log("Theme architecture check passed.");
}
