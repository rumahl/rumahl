import type { BridgeTheme } from "@rumahl/contracts/bridge";

const STYLE_ID = "rumahl-stream-theme";

function themeCss(theme: BridgeTheme): string {
  const palette = theme.palette;
  return [
    `:root{color-scheme:${theme.scheme};`,
    `--rumahl-background:${palette.background};`,
    `--rumahl-surface:${palette.surface};`,
    `--rumahl-text:${palette.text};`,
    `--rumahl-text-muted:${palette.textMuted};`,
    `--rumahl-border:${palette.border};`,
    `--rumahl-accent:${palette.accent};`,
    `--rumahl-on-accent:${palette.onAccent}}`
  ].join("");
}

/**
 * Projects the OS theme into a same-origin streamed app and keeps it in sync.
 *
 * Standalone services (Nextcloud, Plesk, …) embedded by the shell as streams
 * opt in by reading `window.rumahlTheme`, the `--rumahl-*` CSS variables, the
 * `data-rumahl-theme` attribute or the `rumahl:theme` document event. Cross-origin
 * frames are ignored (the access is wrapped and a failure is a no-op).
 */
export function applyStreamTheme(iframe: HTMLIFrameElement | null, theme: BridgeTheme): void {
  if (!iframe) return;
  let doc: Document | null;
  try {
    doc = iframe.contentDocument;
  } catch {
    return;
  }
  const root = doc?.documentElement;
  if (!doc || !root) return;
  root.dataset.rumahlTheme = theme.scheme;
  root.style.setProperty("color-scheme", theme.scheme);
  let style = doc.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!style) {
    style = doc.createElement("style");
    style.id = STYLE_ID;
    (doc.head ?? root).appendChild(style);
  }
  style.textContent = themeCss(theme);
  const view = doc.defaultView;
  if (view) {
    (view as unknown as { rumahlTheme?: BridgeTheme }).rumahlTheme = theme;
    try {
      doc.dispatchEvent(new view.CustomEvent("rumahl:theme", { detail: theme }));
    } catch {
      /* the document went away */
    }
  }
}
