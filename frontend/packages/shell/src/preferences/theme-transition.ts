/**
 * Animates a light/dark mode change with the View Transitions API.
 *
 * The mode is persisted server-side, so the DOM only reflects the new scheme
 * after the save resolves. The transition callback therefore waits until the
 * `.scene.shell[data-scheme]` attribute actually changes (or a short timeout),
 * so the "after" snapshot captures the fully re-themed UI.
 */
interface ViewTransitionLike { finished: Promise<void> }
type StartViewTransition = (callback: () => void | Promise<void>) => ViewTransitionLike;

function startViewTransition(): StartViewTransition | null {
  const start = (document as Document & { startViewTransition?: StartViewTransition }).startViewTransition;
  return typeof start === "function" ? start.bind(document) : null;
}

function scheme(): string | null {
  return document.querySelector(".scene.shell")?.getAttribute("data-scheme") ?? null;
}

function waitForSchemeChange(before: string | null, timeout: number): Promise<void> {
  return new Promise((resolve) => {
    const deadline = performance.now() + timeout;
    const check = () => {
      if (scheme() !== before || performance.now() > deadline) resolve();
      else window.setTimeout(check, 16);
    };
    window.setTimeout(check, 0);
  });
}

/** Runs `update`; when supported, cross-fades the whole UI around the change. */
export function withThemeTransition(update: () => void): void {
  const start = typeof document === "undefined" ? null : startViewTransition();
  if (!start || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
    update();
    return;
  }
  const before = scheme();
  const transition = start(async () => {
    update();
    await waitForSchemeChange(before, 1500);
  });
  transition.finished.catch(() => {});
}
