import { PassThrough, type Readable } from "node:stream";
import { renderToPipeableStream } from "react-dom/server";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { parseShellSnapshot } from "@rumahl/contracts/parse";
import { App } from "@rumahl/shell/App";
import { SHELL_BUILD_ID } from "@rumahl/shell/build-id";
import { EMBEDDED_SNAPSHOT_ID } from "@rumahl/shell/embedded-snapshot";
import { resolveLocale } from "@rumahl/shell/i18n";

export interface ShellAssets {
  script: string;
  stylesheet: string;
}

export interface RenderedShell {
  abort: () => void;
  stream: Readable;
}

const ASSET_PATH = /^\/assets\/[A-Za-z0-9_-]+\.(?:js|css)$/;
const NONCE = /^[A-Za-z0-9_-]{16,128}$/;
export const SSR_SHELL_BUILD_ID = SHELL_BUILD_ID;

export function renderShellDocument(
  value: unknown,
  assets: ShellAssets,
  nonce: string
): Promise<RenderedShell> {
  const snapshot = parseShellSnapshot(value);
  if (snapshot.shellBuildId !== SHELL_BUILD_ID) {
    throw new Error("incompatible shell build");
  }
  if (!ASSET_PATH.test(assets.script) || !assets.script.endsWith(".js") ||
      !ASSET_PATH.test(assets.stylesheet) || !assets.stylesheet.endsWith(".css") ||
      !NONCE.test(nonce)) {
    throw new Error("invalid shell render configuration");
  }

  const embedded = escapeJsonForHtml(snapshot);
  const output = new PassThrough();
  return new Promise((resolve, reject) => {
    let ready = false;
    const renderer = renderToPipeableStream(
      <ShellDocument assets={assets} embedded={embedded} nonce={nonce} snapshot={snapshot} />,
      {
        nonce,
        onShellReady() {
          clearTimeout(deadline);
          ready = true;
          renderer.pipe(output);
          resolve({ stream: output, abort: renderer.abort });
        },
        onShellError(error) {
          clearTimeout(deadline);
          reject(error);
          output.destroy();
        },
        onError(error) {
          if (ready) output.destroy(error instanceof Error ? error : new Error("SSR failed"));
        }
      }
    );
    const deadline = setTimeout(() => {
      renderer.abort();
      reject(new Error("shell render timed out"));
      output.destroy();
    }, 5_000);
  });
}

function ShellDocument({
  assets,
  embedded,
  nonce,
  snapshot
}: {
  assets: ShellAssets;
  embedded: string;
  nonce: string;
  snapshot: ShellSnapshotV1;
}) {
  return (
    <html lang={resolveLocale(snapshot.user.locale)}>
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>rumahl OS</title>
        <link href={assets.stylesheet} rel="stylesheet" />
        <link href={snapshot.theme.stylesheetUrl} rel="stylesheet" />
      </head>
      <body>
        <div data-shell-ssr="1" id="root">
          <App snapshot={snapshot} />
        </div>
        <script
          dangerouslySetInnerHTML={{ __html: embedded }}
          id={EMBEDDED_SNAPSHOT_ID}
          type="application/json"
        />
        <script nonce={nonce} src={assets.script} type="module" />
      </body>
    </html>
  );
}

export function escapeJsonForHtml(value: unknown): string {
  return JSON.stringify(value)
    .replaceAll("<", "\\u003c")
    .replaceAll(">", "\\u003e")
    .replaceAll("&", "\\u0026")
    .replaceAll("\u2028", "\\u2028")
    .replaceAll("\u2029", "\\u2029");
}
