export type UiReadyStatus = "ready" | "degraded";

const STYLE_TOKEN = "--rumahl-ui-color-text-primary";
const DEFAULT_TIMEOUT = 1800;

function stylesheetLinks(): HTMLLinkElement[] {
  return Array.from(document.querySelectorAll<HTMLLinkElement>('link[rel="stylesheet"]'));
}

function stylesheetsReady(links: HTMLLinkElement[]): boolean {
  return links.every((link) => {
    if (!link.sheet) return false;
    try {
      // A parsed stylesheet must expose rules. Cross-origin stylesheets throw
      // here and still count as loaded because their rules are unreadable.
      return link.sheet.cssRules.length > 0;
    } catch {
      return true;
    }
  });
}

function criticalStylesReady(): boolean {
  const value = getComputedStyle(document.documentElement).getPropertyValue(STYLE_TOKEN).trim();
  return value.length > 0;
}

function sleepTick(): Promise<void> {
  // Use a timer rather than rAF so a backgrounded tab still reaches the
  // deadline instead of keeping the splash forever.
  return new Promise((resolve) => setTimeout(resolve, 16));
}

/**
 * Waits for the shell stylesheets and critical theme tokens before the splash
 * reveals the desktop. A slow or broken stylesheet must never trap the user, so
 * the result is marked degraded after the deadline and boot continues.
 */
export async function waitForUiStyles(timeout = DEFAULT_TIMEOUT): Promise<UiReadyStatus> {
  if (typeof document === "undefined") return "ready";
  document.documentElement.dataset.uiReady = "pending";

  const links = stylesheetLinks();
  if (links.length === 0) {
    document.documentElement.dataset.uiReady = "ready";
    return "ready";
  }

  const started = performance.now();
  let sheetsReady = false;
  let tokensReady = false;
  while (performance.now() - started < timeout) {
    sheetsReady = stylesheetsReady(links);
    tokensReady = criticalStylesReady();
    if (sheetsReady && tokensReady) break;
    await sleepTick();
  }

  if (!sheetsReady || !tokensReady) {
    document.documentElement.dataset.uiReady = "degraded";
    return "degraded";
  }

  // Fonts are part of the visual result but must not extend the boot beyond
  // the deadline. If they are still loading, the styles are still usable.
  if (document.fonts?.ready) {
    const remaining = Math.max(0, timeout - (performance.now() - started));
    await Promise.race([
      document.fonts.ready,
      new Promise<void>((resolve) => setTimeout(resolve, Math.min(500, remaining)))
    ]).catch(() => {});
  }

  const status: UiReadyStatus = stylesheetsReady(links) && criticalStylesReady() ? "ready" : "degraded";
  document.documentElement.dataset.uiReady = status;
  return status;
}
