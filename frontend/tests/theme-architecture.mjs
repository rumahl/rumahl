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

function channel(value) {
  const v = value / 255;
  return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function contrast(color, background) {
  const parse = (value) => {
    const match = /^rgba?\(([^)]+)\)$/i.exec(value);
    if (!match) return null;
    return match[1].split(/[,\s/]+/).filter(Boolean).slice(0, 3).map(Number);
  };
  const [r, g, b] = parse(color) ?? [];
  const [br, bg, bb] = parse(background) ?? [];
  if ([r, g, b, br, bg, bb].some((value) => !Number.isFinite(value))) return 0;
  const foreground = 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
  const behind = 0.2126 * channel(br) + 0.7152 * channel(bg) + 0.0722 * channel(bb);
  return (Math.max(foreground, behind) + 0.05) / (Math.min(foreground, behind) + 0.05);
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

  const scaledText = await page.evaluate(() => {
    const root = document.documentElement;
    root.style.setProperty("--rumahl-ui-typography-scale", "1.25");
    const overflow = Math.max(
      document.documentElement.scrollWidth - window.innerWidth,
      document.body.scrollWidth - window.innerWidth
    );
    root.style.removeProperty("--rumahl-ui-typography-scale");
    return { overflow };
  });
  check("text-scale-overflow", scaledText.overflow <= 1, scaledText);

  const motion = await page.evaluate(() => {
    const shell = document.querySelector(".shell");
    if (!(shell instanceof HTMLElement)) return null;
    const button = document.createElement("button");
    button.className = "btn";
    shell.append(button);
    shell.style.setProperty("--motion-control-duration", "40ms");
    const duration = getComputedStyle(button).transitionDuration;
    shell.style.removeProperty("--motion-control-duration");
    button.remove();
    return { duration };
  });
  check("motion-role", motion !== null && motion.duration.includes("0.04s"), motion);

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

    // Menus live in a body portal. They must receive the same recipe context
    // through :root, not only through the .shell subtree.
    const portalMenu = document.createElement("div");
    portalMenu.className = "desktop-menu theme-architecture-portal";
    document.body.append(portalMenu);
    root.style.setProperty("--menu-background", "rgb(4, 5, 6)");
    const portalBackground = getComputedStyle(portalMenu).backgroundColor;
    root.style.removeProperty("--menu-background");
    portalMenu.remove();

    const result = { before, afterRadius, afterTokenRadius, afterRecipeRadius, afterMaterial, portalBackground };
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
  check("ground-portal-menu", ground.portalBackground === "rgb(4, 5, 6)", ground);

  // With a CSS-gradient wallpaper there is no image to sample. Labels must
  // fall back to the semantic on-wallpaper ink instead of hard-coded white.
  const labelPrepared = await page.evaluate(() => {
    document.querySelectorAll(".wallpaper-image, .wallpaper-video").forEach((element) => element.remove());
    const root = document.documentElement;
    const shell = document.querySelector(".shell");
    root.dataset.themeBase = "ground";
    if (shell instanceof HTMLElement) shell.dataset.themeBase = "ground";
    root.style.setProperty("--rumahl-ui-color-on-wallpaper", "#000000");
    const wallpaper = document.querySelector(".wallpaper");
    if (wallpaper instanceof HTMLElement) {
      wallpaper.style.backgroundImage = "linear-gradient(rgb(255, 255, 255), rgb(255, 255, 255))";
      wallpaper.style.backgroundColor = "#ffffff";
    }
    const label = document.querySelector(".desktop-icon__label");
    if (!(label instanceof HTMLElement)) return false;
    label.style.setProperty("--rumahl-ui-color-on-wallpaper", "#000000");
    label.style.removeProperty("--adaptive-ink");
    label.style.removeProperty("--adaptive-muted");
    label.style.removeProperty("--adaptive-shadow");
    return true;
  });
  // One sampler tick plus the deliberate 1.4s adaptive-colour transition.
  await page.waitForTimeout(2800);
  const labelFallback = await page.evaluate(() => {
    const label = document.querySelector(".desktop-icon__label");
    return label instanceof HTMLElement ? { color: getComputedStyle(label).color } : null;
  });
  check(
    "ground-label-fallback-contrast",
    labelPrepared && labelFallback !== null && contrast(labelFallback.color, "rgb(255, 255, 255)") >= 4.5,
    labelFallback
  );

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
