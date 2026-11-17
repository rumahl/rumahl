/**
 * Real-shell validation for Ground and derived themes.
 *
 * Installs the bundled example themes plus a custom Ground theme, selects each
 * through the device preference API, and checks the rendered shell (window,
 * dock, menu, widget, typography and material) rather than token values alone.
 *
 * Usage:
 *   pnpm --dir frontend ground-themes
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const base = (process.env.RUMAHL_AUDIT_URL ?? "https://localhost:8443").replace(/\/$/, "");
const credentialsPath = process.env.RUMAHL_AUDIT_CREDENTIALS ?? resolve(here, "../../.rumahl-dev/credentials.json");
const insecure = process.env.RUMAHL_AUDIT_INSECURE !== "0";
const executablePath = process.env.RUMAHL_AUDIT_CHROME ?? "/usr/bin/google-chrome";
const outDir = process.env.RUMAHL_VISUAL_OUT ?? resolve(here, "../.shots/phase2");
const themesDir = resolve(here, "../packages/shell/themes");
const failures = [];
const results = [];

const credentials = (() => {
  if (process.env.RUMAHL_AUDIT_USER && process.env.RUMAHL_AUDIT_PASSWORD) {
    return { username: process.env.RUMAHL_AUDIT_USER, password: process.env.RUMAHL_AUDIT_PASSWORD };
  }
  const parsed = JSON.parse(readFileSync(credentialsPath, "utf8"));
  if (typeof parsed.username === "string" && typeof parsed.password === "string") return parsed;
  throw new Error(`No Ground-theme credentials at ${credentialsPath}.`);
})();

function check(name, condition, detail) {
  if (!condition) failures.push({ name, detail });
}

const custom = {
  id: "com.test.ground-custom",
  name: "Ground Custom Probe",
  version: "1.0.0",
  base: "ground",
  tokens: {
    "color.accent": "#ff5500",
    "color.accent.strong": "#ff8844",
    "typography.family": "Georgia, serif",
    "typography.display": "Georgia, serif",
    "shape.radius.window": "10px",
    "shape.radius.dock": "14px",
    "shape.radius.icon": "8px"
  },
  css: ":root { --menu-background: rgb(1, 2, 3); --menu-radius: 4px; }"
};

const scenarios = [
  { id: "com.rumahl.default", name: "iris", install: null, base: "iris", accent: "#28694c", window: "23px", dock: "28px", menuRadius: "23px" },
  { id: "com.rumahl.core", name: "core", install: "core.json", base: "ground", accent: "#1a73e8", window: "0px", dock: "0px", menuRadius: "0px" },
  { id: "com.example.dashboard", name: "dashboard", install: "dashboard.json", base: "ground", accent: "#4f46e5", window: "18px", dock: "18px", menuRadius: "0px" },
  { id: "com.example.terminal", name: "terminal", install: "terminal.json", base: "ground", accent: "#39ff14", window: "0px", dock: "0px", menuRadius: "0px" },
  {
    id: "com.example.meridian",
    name: "meridian",
    install: "meridian.json",
    base: "ground",
    accent: "#c6a15b",
    window: "3px",
    dock: "2px",
    menuRadius: "3px",
    menuBackground: "rgb(13, 18, 25)",
    widgetBackground: "rgb(23, 31, 41)",
    windowShadow: /30px 80px/,
    bodyFont: /Manrope/,
    displayFont: /Playfair/,
    taskbar: true
  },
  {
    id: "com.example.retro",
    name: "retro",
    install: "retro.json",
    base: "ground",
    accent: "#b23a2b",
    window: "0px",
    dock: "0px",
    menuRadius: "0px",
    menuBackground: "rgb(255, 248, 231)",
    widgetBackground: "rgb(255, 248, 231)",
    dockBackground: "rgb(243, 228, 196)",
    windowShadow: /6px 6px 0px/,
    bodyFont: /Courier New/i,
    displayFont: /Instrument Serif/i
  },
  { id: custom.id, name: "custom", install: "__custom__", base: "ground", accent: "#ff5500", window: "10px", dock: "14px", menuRadius: "4px" }
];

mkdirSync(outDir, { recursive: true });
const customPath = resolve(outDir, "ground-custom.json");
writeFileSync(customPath, JSON.stringify(custom, null, 2));

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
  await page.waitForTimeout(2200);

  const device = await page.evaluate(() => localStorage.getItem("rumahl.browser-profile.v1"));
  const install = async (file, id) => {
    await page.goto(`${base}/settings/display/themes`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(1500);
    await page.setInputFiles('input[type="file"]', file);
    await page
      .waitForFunction(
        (themeId) => (localStorage.getItem("rumahl.installed-themes") ?? "").includes(themeId),
        id,
        { timeout: 12_000 }
      )
      .catch(() => {});
    await page.waitForTimeout(600);
  };

  // Install every package once while the shell is still on Iris. Selecting a
  // theme later must not depend on the theme manager page rendering under that
  // theme (dashboard replaces the desktop, terminal restyles every control).
  const installs = scenarios.filter((scenario) => scenario.install !== null);
  for (const scenario of installs) {
    const path = scenario.install === "__custom__" ? customPath : resolve(themesDir, scenario.install);
    await install(path, scenario.id);
    const installed = await page.evaluate((id) => (localStorage.getItem("rumahl.installed-themes") ?? "").includes(id), scenario.id);
    check(`${scenario.name}-installed`, installed, scenario.id);
  }

  for (const scenario of scenarios) {
    let selected = false;
    let lastPrefs = null;
    for (let attempt = 0; attempt < 6 && !selected; attempt += 1) {
      const prefs = await page.evaluate(async (device) => {
        const response = await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`);
        return response.json();
      }, device);
      const response = await page.evaluate(async ({ device, revision, id }) => {
        const result = await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ settingsVersion: 1, revision, scope: "user", key: "shell.theme", value: id })
        });
        return { status: result.status, body: await result.text() };
      }, { device, revision: prefs.revision, id: scenario.id });
      if (response.status === 200) {
        lastPrefs = JSON.parse(response.body);
        if (lastPrefs.effective?.shellTheme === scenario.id) {
          selected = true;
          break;
        }
        // The installer may have left a device-scoped override. Pin the test
        // device to the scenario as well, then fall through to a retry.
        const deviceResponse = await page.evaluate(async ({ device, revision, id }) => {
          const result = await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ settingsVersion: 1, revision, scope: "device", key: "shell.theme", value: id })
          });
          return { status: result.status, body: await result.text() };
        }, { device, revision: lastPrefs.revision, id: scenario.id });
        if (deviceResponse.status === 200) {
          lastPrefs = JSON.parse(deviceResponse.body);
          selected = lastPrefs.effective?.shellTheme === scenario.id;
          if (selected) break;
        }
      }
      await page.waitForTimeout(400);
    }
    check(`${scenario.name}-selected`, selected, lastPrefs?.effective?.shellTheme ?? scenario.id);

    await page.goto(`${base}/`, { waitUntil: "domcontentloaded" });
    await page.waitForFunction((id) => document.querySelector(".shell")?.dataset.theme === id, scenario.id, { timeout: 20_000 }).catch(() => {});
    await page.waitForTimeout(700);
    for (let i = 0; i < 8; i += 1) {
      const close = page.locator(".window-control--close").first();
      if (!(await close.count())) break;
      await close.click({ force: true }).catch(() => {});
      await page.waitForTimeout(120);
    }
    await page.waitForTimeout(350);
    // Session restore is timing dependent once several themes have run. Open a
    // window deterministically so every scenario audits real window chrome.
    if (!(await page.locator(".shell-window").count())) {
      const icon = page.locator(".desktop-icon", { hasText: "Files" }).first();
      if (await icon.count()) {
        await icon.dblclick().catch(() => {});
        await page.waitForTimeout(1100);
      }
    }

    const desktop = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const shell = document.querySelector(".shell");
      const pick = (selector) => {
        const element = document.querySelector(selector);
        if (!element) return null;
        const style = getComputedStyle(element);
        return {
          color: style.color,
          background: style.backgroundColor,
          border: style.borderTopColor,
          radius: style.borderRadius,
          font: style.fontFamily,
          shadow: style.boxShadow,
          backdrop: style.backdropFilter
        };
      };
      return {
        theme: shell?.dataset.theme,
        base: shell?.dataset.themeBase,
        accent: root.getPropertyValue("--rumahl-ui-color-accent").trim(),
        font: root.getPropertyValue("--rumahl-ui-typography-family").trim(),
        displayFont: root.getPropertyValue("--rumahl-ui-typography-display").trim(),
        shapeWindow: root.getPropertyValue("--rumahl-ui-shape-radius-window").trim(),
        shapeDock: root.getPropertyValue("--rumahl-ui-shape-radius-dock").trim(),
        window: pick(".shell-window"),
        taskbar: pick(".taskbar"),
        dock: pick(".os-dock"),
        topbar: pick(".topbar__island"),
        widget: pick(".desktop-widget"),
        bodyFont: getComputedStyle(document.body).fontFamily,
        labelInk: (() => {
          const element = document.querySelector(".desktop-icon__label");
          return element ? getComputedStyle(element).color : null;
        })(),
        widgetEyebrow: (() => {
          const element = document.querySelector(".desktop-widget__eyebrow");
          return element ? getComputedStyle(element).color : null;
        })(),
        menubar: (() => {
          const element = document.querySelector(".menubar.topbar");
          if (!element) return null;
          const style = getComputedStyle(element);
          return { background: style.backgroundColor, border: style.borderTopWidth };
        })(),
        wallpaper: (() => {
          const element = document.querySelector(".wallpaper");
          if (!element) return null;
          const rect = element.getBoundingClientRect();
          return { top: rect.top, height: rect.height };
        })(),
        spotlight: (() => {
          const element = document.querySelector(".menubar__spotlight svg");
          return element
            ? { viewBox: element.getAttribute("viewBox"), html: element.innerHTML.slice(0, 200) }
            : null;
        })(),
        overflow: document.documentElement.scrollWidth - window.innerWidth
      };
    });

    const menu = await (async () => {
      const go = page.locator(".menubar button", { hasText: "Go" }).first();
      if (!(await go.count())) return null;
      await go.click();
      await page.waitForTimeout(300);
      const value = await page.evaluate(() => {
        const element = document.querySelector(".desktop-menu, .menubar__dropdown, .topbar__dropdown");
        if (!element) return null;
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, radius: style.borderRadius, shadow: style.boxShadow, backdrop: style.backdropFilter };
      });
      await page.keyboard.press("Escape");
      return value;
    })();

    await page.screenshot({ path: resolve(outDir, `${scenario.name}-desktop.png`) });
    await page.goto(`${base}/app/files`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(1200);
    await page.screenshot({ path: resolve(outDir, `${scenario.name}-files.png`) });

    check(`${scenario.name}-theme`, desktop.theme === scenario.id, desktop.theme);
    check(`${scenario.name}-base`, desktop.base === scenario.base, desktop.base);
    check(`${scenario.name}-accent-token`, desktop.accent.toLowerCase() === scenario.accent.toLowerCase(), desktop.accent);
    check(`${scenario.name}-window-token`, desktop.shapeWindow === scenario.window, desktop.shapeWindow);
    check(`${scenario.name}-dock-token`, desktop.shapeDock === scenario.dock, desktop.shapeDock);
    check(`${scenario.name}-window-radius`, desktop.window?.radius === scenario.window || desktop.window === null, desktop.window);
    check(`${scenario.name}-dock-radius`, desktop.dock?.radius === scenario.dock || desktop.dock === null, desktop.dock);
    check(`${scenario.name}-overflow`, desktop.overflow <= 1, desktop.overflow);
    // The floating top bar is a transparent strip: only its islands paint.
    // An opaque bar would leave a dead strip above the wallpaper.
    check(
      `${scenario.name}-topbar-transparent`,
      desktop.menubar?.background === "rgba(0, 0, 0, 0)" && desktop.menubar?.border === "0px",
      desktop.menubar
    );
    check(
      `${scenario.name}-wallpaper-full`,
      desktop.wallpaper?.top === 0 && Math.round(desktop.wallpaper?.height ?? 0) === 900,
      desktop.wallpaper
    );
    check(`${scenario.name}-menu`, menu !== null && (scenario.name === "iris" || menu.radius === scenario.menuRadius), menu);

    if (scenario.base === "ground") {
      check(`${scenario.name}-system-typography`, !/Inter|Space Grotesk/i.test(desktop.bodyFont), desktop.bodyFont);
      check(`${scenario.name}-neutral-topbar`, !/rgb\(40, 105, 76\)|rgb\(165, 138, 255\)/i.test(`${desktop.topbar?.background ?? ""} ${desktop.topbar?.border ?? ""}`), desktop.topbar);
      if (scenario.name !== "dashboard") {
        check(`${scenario.name}-solid-widget`, desktop.widget !== null && (desktop.widget.backdrop === "none" || desktop.widget.backdrop === ""), desktop.widget);
      }
    }

    if (scenario.name === "core") {
      check("core-square", desktop.shapeWindow === "0px" && desktop.shapeDock === "0px", desktop);
      check("core-neutral-accent", desktop.accent === "#1a73e8", desktop.accent);
    }
    if (scenario.name === "dashboard") {
      check("dashboard-type", /Manrope/i.test(desktop.bodyFont), desktop.bodyFont);
      check("dashboard-shape", desktop.shapeWindow === "18px" && desktop.shapeDock === "18px", desktop);
    }
    if (scenario.name === "terminal") {
      check("terminal-type", /Courier New/i.test(desktop.bodyFont), desktop.bodyFont);
      check("terminal-accent", desktop.accent === "#39ff14", desktop.accent);
      check("terminal-menu-material", menu?.background === "rgb(0, 0, 0)", menu);
      check("terminal-widget-material", desktop.widget?.background === "rgb(0, 0, 0)", desktop.widget);
    }
    if (scenario.name === "meridian") {
      check("meridian-type", scenario.bodyFont.test(desktop.bodyFont), desktop.bodyFont);
      check("meridian-display-type", scenario.displayFont.test(desktop.displayFont), desktop.displayFont);
      check("meridian-taskbar", desktop.taskbar !== null, desktop.taskbar);
      check("meridian-window-shadow", scenario.windowShadow.test(desktop.window?.shadow ?? ""), desktop.window);
      check("meridian-dock-absent", desktop.dock === null, desktop.dock);
      check("meridian-widget-material", desktop.widget?.background === scenario.widgetBackground, desktop.widget);
      check("meridian-menu-material", menu?.background === scenario.menuBackground, menu);
    }
    if (scenario.name === "retro") {
      check("retro-type", scenario.bodyFont.test(desktop.bodyFont), desktop.bodyFont);
      check("retro-display-type", scenario.displayFont.test(desktop.displayFont), desktop.displayFont);
      check("retro-opaque", desktop.window?.backdrop === "none", desktop.window);
      check("retro-window-shadow", scenario.windowShadow.test(desktop.window?.shadow ?? ""), desktop.window);
      check("retro-dock-background", desktop.dock?.background === scenario.dockBackground, desktop.dock);
      check("retro-widget-material", desktop.widget?.background === scenario.widgetBackground, desktop.widget);
      check("retro-menu-material", menu?.background === scenario.menuBackground, menu);
      check("retro-menu-shadow", /4px 4px 0px/.test(menu?.shadow ?? ""), menu);
      // A theme-owned opaque label surface must replace the wallpaper ink,
      // and the widget eyebrow must stay readable on the solid widget.
      check("retro-label-ink", desktop.labelInk === "rgb(44, 36, 27)", desktop.labelInk);
      check("retro-widget-eyebrow", desktop.widgetEyebrow === "rgb(107, 93, 79)", desktop.widgetEyebrow);
      // The retro icon pack replaces the stroked circle with a square lens.
      check("retro-icon-pack", /<rect/.test(desktop.spotlight?.html ?? ""), desktop.spotlight);
    }
    if (scenario.name === "custom") {
      check("custom-type", /Georgia/i.test(desktop.bodyFont), desktop.bodyFont);
      check("custom-window", desktop.window?.radius === "10px", desktop.window);
      check("custom-dock", desktop.dock?.radius === "14px", desktop.dock);
      check("custom-menu", menu?.background === "rgb(1, 2, 3)" && menu?.radius === "4px", menu);
    }

    results.push({ scenario: scenario.name, desktop, menu });
  }

  // Always leave the account on Iris.
  const prefs = await page.evaluate(async (device) => {
    const response = await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`);
    return response.json();
  }, device);
  await page.evaluate(async ({ device, revision }) => {
    const response = await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ settingsVersion: 1, revision, scope: "user", key: "shell.theme", value: "com.rumahl.default" })
    });
    if (!response.ok) return;
    const current = await response.json();
    await fetch(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ settingsVersion: 1, revision: current.revision, scope: "device", key: "shell.theme", value: null })
    });
  }, { device, revision: prefs.revision });
  await page.goto(`${base}/`, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1200);
  writeFileSync(resolve(outDir, "ground-themes.json"), JSON.stringify(results, null, 2));
  await context.close();
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(`\nGround theme validation failed: ${failures.length} issue(s)\n`);
  for (const failure of failures) console.error(`[${failure.name}] ${JSON.stringify(failure.detail)}`);
  process.exitCode = 1;
} else {
  console.log(`Ground theme validation passed. Screenshots: ${outDir}`);
}
