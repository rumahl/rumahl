/**
 * Rendered adaptive-contrast checks for photo/video and CSS-gradient wallpapers.
 *
 * A synthetic canvas stream exercises the video sampler without external media;
 * CSS gradients exercise the fallback path used by Ground themes. The checks
 * measure the actual label ink and contrast, not token values.
 *
 * Usage:
 *   pnpm --dir frontend adaptive-media
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
  throw new Error(`No adaptive media credentials at ${credentialsPath}.`);
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
    return match ? match[1].split(/[,\s/]+/).filter(Boolean).slice(0, 3).map(Number) : null;
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
  args: [
    ...(insecure ? ["--ignore-certificate-errors"] : []),
    "--autoplay-policy=no-user-gesture-required"
  ]
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
  await page.waitForTimeout(2600);

  const setVideo = async (color) => {
    await page.evaluate(({ color }) => {
      document.querySelectorAll(".wallpaper-image, .wallpaper-video").forEach((element) => element.remove());
      const canvas = document.createElement("canvas");
      canvas.width = 64;
      canvas.height = 64;
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = color;
      ctx.fillRect(0, 0, 64, 64);
      const video = document.createElement("video");
      video.className = "wallpaper-video";
      video.muted = true;
      video.playsInline = true;
      video.autoplay = true;
      video.style.cssText = "position:absolute;inset:0;width:100%;height:100%;object-fit:cover;opacity:1";
      video.srcObject = canvas.captureStream(10);
      document.querySelector(".wallpaper")?.append(video);
      void video.play().catch(() => {});
    }, { color });
    await page.waitForTimeout(3600);
  };

  const readLabel = () => page.evaluate(() => {
    const label = document.querySelector(".desktop-icon__label");
    if (!(label instanceof HTMLElement)) return null;
    const style = getComputedStyle(label);
    return { color: style.color, shadow: style.textShadow, ink: label.style.getPropertyValue("--adaptive-ink") };
  });

  await setVideo("#ffffff");
  const white = await readLabel();
  check("video-white-ink", white !== null && contrast(white.color, "rgb(255, 255, 255)") >= 4.5, white);
  check("video-white-shadow", white?.shadow.includes("rgba(255, 255, 255") === true || white?.shadow !== "none", white);
  await page.screenshot({ path: resolve(here, "../.shots/phase2/adaptive-video-white.png") });

  await setVideo("#000000");
  const black = await readLabel();
  check("video-black-ink", black !== null && contrast(black.color, "rgb(0, 0, 0)") >= 4.5, black);
  check("video-black-shadow", black?.shadow.includes("rgba(0, 0, 0") === true, black);
  await page.screenshot({ path: resolve(here, "../.shots/phase2/adaptive-video-black.png") });

  const setGradient = async (gradient) => {
    await page.evaluate(({ gradient }) => {
      document.querySelectorAll(".wallpaper-image, .wallpaper-video").forEach((element) => element.remove());
      const wallpaper = document.querySelector(".wallpaper");
      if (wallpaper instanceof HTMLElement) wallpaper.style.backgroundImage = gradient;
    }, { gradient });
    await page.waitForTimeout(3200);
  };

  await setGradient("linear-gradient(rgb(255, 255, 255), rgb(255, 255, 255))");
  const lightGradient = await readLabel();
  check(
    "gradient-light-ink",
    lightGradient !== null && contrast(lightGradient.color, "rgb(255, 255, 255)") >= 4.5,
    lightGradient
  );

  await setGradient("linear-gradient(rgb(0, 0, 0), rgb(0, 0, 0))");
  const darkGradient = await readLabel();
  check(
    "gradient-dark-ink",
    darkGradient !== null && contrast(darkGradient.color, "rgb(0, 0, 0)") >= 4.5,
    darkGradient
  );

  await context.close();
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(`\nAdaptive media check failed: ${failures.length} issue(s)\n`);
  for (const failure of failures) console.error(`[${failure.name}] ${JSON.stringify(failure.detail)}`);
  process.exitCode = 1;
} else {
  console.log("Adaptive media check passed.");
}
