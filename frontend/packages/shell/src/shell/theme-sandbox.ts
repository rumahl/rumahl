import type { Theme } from "@rumahl/ui/themes";
import type { Tokens } from "@rumahl/ui/tokens";

/**
 * Sandboxed theme runtime.
 *
 * Theme code never enters the shell's JavaScript realm here. It runs in an
 * opaque-origin iframe (`sandbox="allow-scripts"`, no `allow-same-origin`) and
 * draws inside its own document. Its CSP allows inline styles, the bootstrap
 * nonce and a blob module; network, forms, navigation and host resources are
 * blocked. The shell only sizes and positions the layer.
 */

const SANDBOX_STYLE = "html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent;pointer-events:none}*,*::before,*::after{box-sizing:border-box}";

const BOOTSTRAP = `
const data = JSON.parse(document.getElementById("rumahl-theme-data").textContent);
const url = URL.createObjectURL(new Blob([data.code], { type: "text/javascript" }));
try {
  const mod = await import(url);
  const activate = mod.default ?? mod.activate;
  if (typeof activate === "function") {
    const cleanup = activate({ root: document.documentElement, layer: data.layer, state: data.state });
    if (typeof cleanup === "function") window.addEventListener("pagehide", () => cleanup(), { once: true });
  }
} catch (error) {
  console.error("[theme]", error);
} finally {
  URL.revokeObjectURL(url);
}
`;

function nonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2);
}

function sandboxDocument(
  code: string,
  layer: "background" | "overlay",
  tokens: Tokens,
  animations: boolean,
  scriptNonce: string,
): string {
  // Escaping `<` keeps a theme source containing `</script>` inert inside the
  // JSON data block.
  const payload = JSON.stringify({ code, layer, state: { tokens, animations } })
    .replaceAll("<", "\\u003c");
  const policy = [
    "default-src 'none'",
    "style-src 'unsafe-inline'",
    `script-src 'nonce-${scriptNonce}' blob:`,
    "img-src data: blob:",
    "font-src data:",
    "media-src data: blob:",
    "connect-src 'none'",
    "form-action 'none'",
    "base-uri 'none'",
  ].join("; ");
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${policy}"><style>${SANDBOX_STYLE}</style></head><body><script type="application/json" id="rumahl-theme-data">${payload}</script><script type="module" nonce="${scriptNonce}">${BOOTSTRAP}</script></body></html>`;
}

/**
 * Mounts one sandboxed iframe per theme layer. Returns a cleanup function.
 * Without URL.createObjectURL (for example in jsdom) it is a safe no-op.
 */
export function mountSandboxTheme(
  theme: Theme,
  tokens: Tokens,
  animations: boolean,
  background: HTMLElement,
  overlay: HTMLElement,
): () => void {
  if (!theme.code || typeof URL.createObjectURL !== "function") return () => undefined;
  const code = theme.code;
  const scriptNonce = nonce();
  const urls: string[] = [];
  const frames: HTMLIFrameElement[] = [];
  const layers: Array<["background" | "overlay", HTMLElement]> = [
    ["background", background],
    ["overlay", overlay],
  ];
  for (const [layer, container] of layers) {
    const html = sandboxDocument(code, layer, tokens, animations, scriptNonce);
    const url = URL.createObjectURL(new Blob([html], { type: "text/html" }));
    const frame = document.createElement("iframe");
    frame.className = "theme-sandbox-frame";
    frame.setAttribute("sandbox", "allow-scripts");
    frame.setAttribute("referrerpolicy", "no-referrer");
    frame.setAttribute("aria-hidden", "true");
    frame.tabIndex = -1;
    frame.title = `Theme ${layer}`;
    frame.src = url;
    urls.push(url);
    frames.push(frame);
    container.replaceChildren(frame);
  }
  return () => {
    frames.forEach((frame) => frame.remove());
    urls.forEach((url) => URL.revokeObjectURL(url));
    background.replaceChildren();
    overlay.replaceChildren();
  };
}
