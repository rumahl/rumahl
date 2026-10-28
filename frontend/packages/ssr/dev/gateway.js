import console from "node:console";
import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { createServer as createHttpsServer } from "node:https";
import { request as httpRequest } from "node:http";
import { connect } from "node:net";
import { chmod, readFile } from "node:fs/promises";
import { join } from "node:path";
import { URL } from "node:url";
import { Readable } from "node:stream";
import { createServer as createViteServer } from "vite";
import { createRendererServer } from "../renderer-http.js";

const NONCE_PLACEHOLDER = "RUMAHL_DEV_NONCE_PLACEHOLDER";
const assets = { script: "/assets/rumahl-dev.js", stylesheet: "/assets/rumahl-dev.css" };
const frontendPath = /^\/(?:@vite\/|@react-refresh|@id\/|@fs\/|src\/|node_modules\/|assets\/rumahl-dev\.)/;

/**
 * Minimal, self-contained recovery environment served on the normal port while
 * the platform is unreachable. It reconnects automatically once the platform is
 * back, so the RRE is always reachable even when everything else is down.
 */
function recoveryFallback() {
  // Plain, calm screen: black, the white rumahl "r" top right, a spinner first
  // and an error only after the platform keeps failing. It reconnects itself.
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>rumahl · Recovery</title><style>
:root{color-scheme:dark}*{box-sizing:border-box}html,body{height:100%}body{margin:0;background:#000;color:#fff;font:15px/1.6 ui-sans-serif,system-ui,"Segoe UI",Roboto,sans-serif;display:grid;place-items:center;padding:32px}
.mark{position:fixed;top:24px;right:28px;width:24px;height:auto}
.center{max-width:440px;display:grid;gap:16px;justify-items:center;text-align:center}
.spinner{width:30px;height:30px;border-radius:50%;border:2px solid rgba(255,255,255,.14);border-top-color:#fff;animation:s .8s linear infinite}
@keyframes s{to{transform:rotate(360deg)}}
h1{margin:0;font-size:21px;font-weight:600;letter-spacing:-.01em}
p{margin:0;color:#9aa2ae;font-size:14px}
.actions{display:flex;gap:10px;flex-wrap:wrap;justify-content:center;margin-top:4px}
.actions button{border:1px solid rgba(255,255,255,.15);background:rgba(255,255,255,.06);color:#fff;border-radius:10px;padding:9px 16px;font:inherit;cursor:pointer}
[hidden]{display:none!important}
</style></head><body><svg class="mark" viewBox="0 0 251 472" fill="currentColor" aria-hidden="true"><path d="M 9.47163 471.21342 Q 0 471.21342 0 461.74179 V 203.63996 C 0 80.50881 80.50883 0 201.27207 0 H 241.52649 Q 250.99811 0 250.99811 9.47163 V 94.71626 Q 250.99811 104.18789 241.52649 104.18789 H 210.74370 C 144.44231 104.18789 101.81999 144.44230 101.81999 210.74369 V 461.74179 Q 101.81999 471.21342 92.34836 471.21342 Z"/></svg><div class="center"><div class="spinner" id="sp"></div><h1 id="t">Recovering rumahl</h1><p id="d">One moment while the system is checked…</p><div class="actions" id="a"></div></div><script>(function(){var n=0,t=document.getElementById("t"),d=document.getElementById("d"),s=document.getElementById("sp"),a=document.getElementById("a");function fail(){s.hidden=true;t.textContent="rumahl is not reachable right now";d.textContent="A central component is not responding. The system keeps being monitored.";a.replaceChildren();var r=document.createElement("button");r.textContent="Try again";r.onclick=function(){n=0;s.hidden=false;t.textContent="Recovering rumahl";d.textContent="One moment while the system is checked…";a.replaceChildren();poll();};a.appendChild(r);}function poll(){fetch("/",{method:"GET",cache:"no-store",credentials:"same-origin"}).then(function(r){var c=r.headers.get("content-type")||"";if(r.status===200&&c.indexOf("text/html")===0){location.assign("/");return;}n++;if(n>=6)fail();setTimeout(poll,1500);}).catch(function(){n++;if(n>=6)fail();setTimeout(poll,1500);});}poll();})();</script></body></html>`;
}

function wantsHtml(request) {
  if (request.method !== "GET") return false;
  const accept = String(request.headers.accept || "");
  return accept.includes("text/html") || accept === "" || accept.includes("*/*");
}

export async function startGateway({ repo, runtime, state, cert, key, port, buildId, polling }) {
  const platformSocket = join(runtime, "gateway.sock");
  // The shared, self-contained offline/recovery status screen (black, white "r",
  // spinner first, then tips). Serving the same file keeps dev and production in
  // step; the inline fallback only covers a missing checkout.
  const offlineHtml = await readFile(join(repo, "platform-web/src/offline/index.html"), "utf8").catch(() => recoveryFallback());
  let vite;
  let origin;
  const edge = createHttpsServer({ cert: await readFile(cert), key: await readFile(key) }, (request, response) => {
    const shellHost = new URL(origin).host;
    const appHost = new RegExp(`^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\\.apps\\.localhost:${new URL(origin).port}$`).test(request.headers.host ?? "");
    if (request.headers.host !== shellHost && !appHost) {
      response.writeHead(421).end();
      return;
    }
    // Vite serves source code, so never permit cross-origin browser access.
    if (!appHost && request.headers.origin && request.headers.origin !== origin) {
      response.writeHead(403).end();
      return;
    }
    const path = request.url.split("?")[0];
    if (appHost) {
      if (!["GET", "HEAD"].includes(request.method)) { response.writeHead(405).end(); return; }
      delete request.headers.cookie;
      delete request.headers.authorization;
    }
    if (!appHost && frontendPath.test(path)) {
      if (!["GET", "HEAD"].includes(request.method)) {
        response.writeHead(405).end();
        return;
      }
      response.setHeader("Cache-Control", "no-store");
      if (path === assets.script || path === assets.stylesheet) {
        response.writeHead(307, { Location: path === assets.script ? "/src/main.tsx" : "/src/styles.css" }).end();
        return;
      }
      // Credentials never enter Vite middleware or module hooks.
      delete request.headers.cookie;
      delete request.headers.authorization;
      if (!vite) response.writeHead(503).end();
      else vite.middlewares(request, response, () => response.writeHead(404).end());
      return;
    }
    const headers = { ...request.headers };
    for (const name of Object.keys(headers)) {
      if (name.startsWith("x-rumahl-") || name.startsWith("x-forwarded-") || name === "forwarded") delete headers[name];
    }
    const upstream = httpRequest({ socketPath: platformSocket, method: request.method, path: request.url, headers }, (incoming) => {
      response.writeHead(incoming.statusCode, incoming.headers);
      incoming.pipe(response);
      incoming.on("error", () => response.destroy());
    });
    upstream.setTimeout(15_000, () => upstream.destroy());
    upstream.on("error", () => {
      if (response.headersSent) {
        response.destroy();
      } else if (wantsHtml(request)) {
        response
          .writeHead(503, {
            "Cache-Control": "no-store",
            "Content-Type": "text/html; charset=utf-8",
            "Content-Security-Policy":
              "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self' ws: wss:; manifest-src 'self'; base-uri 'none'",
          })
          .end(offlineHtml);
      } else {
        response
          .writeHead(503, { "Cache-Control": "no-store", "Content-Type": "text/plain" })
          .end("Platform restarting. Please retry shortly.");
      }
    });
    response.on("close", () => upstream.destroy());
    request.on("error", () => upstream.destroy());
    request.pipe(upstream);
  });
  edge.requestTimeout = 15_000;
  edge.headersTimeout = 10_000;

  // Install the origin gate before Vite installs its own upgrade listener.
  edge.on("upgrade", (request, socket, head) => {
    if (request.headers.host !== new URL(origin).host || request.headers.origin !== origin) {
      socket.destroy();
      return;
    }
    if (request.url.split("?")[0] === "/__dev/hmr") {
      delete request.headers.cookie;
      delete request.headers.authorization;
      return; // Vite handles this upgrade with its own per-server token.
    }
    if (request.url !== "/api/v1/shell/events") {
      socket.destroy();
      return;
    }
    const upstream = connect(platformSocket);
    upstream.on("error", () => socket.destroy());
    socket.on("error", () => upstream.destroy());
    socket.on("close", () => upstream.destroy());
    upstream.on("connect", () => {
      const headers = Object.entries(request.headers).filter(([name]) =>
        !name.startsWith("x-rumahl-") && !name.startsWith("x-forwarded-") && name !== "forwarded"
      ).map(([name, value]) => `${name}: ${value}`).join("\r\n");
      upstream.write(`${request.method} ${request.url} HTTP/1.1\r\n${headers}\r\n\r\n`);
      if (head.length) upstream.write(head);
      socket.pipe(upstream).pipe(socket);
    });
  });

  let renderer;
  try {
    // Bind first so port 0 can choose a free port, but serve only after setup.
    await new Promise((resolve, reject) => {
      edge.once("error", reject);
      edge.listen(port, "127.0.0.1", resolve);
    });
    origin = `https://localhost:${edge.address().port}`;
    vite = await createViteServer({
      root: join(repo, "frontend/packages/shell"),
      configFile: join(repo, "frontend/packages/shell/vite.config.ts"),
      appType: "custom",
      mode: "development",
      cacheDir: join(repo, "frontend/node_modules/.vite-rumahl-dev", createHash("sha256").update(state).digest("hex").slice(0, 12)),
      define: { __RUMAHL_SHELL_BUILD_ID__: JSON.stringify(buildId) },
      html: { cspNonce: NONCE_PLACEHOLDER },
      ssr: { noExternal: [/^@rumahl\//] },
      server: {
        middlewareMode: true,
        cors: false,
        allowedHosts: ["localhost"],
        hmr: { server: edge, protocol: "wss", host: "localhost", clientPort: edge.address().port, path: "/__dev/hmr" },
        // `frontend/node_modules` is a symlink in disposable dev copies, so its
        // real path (where fonts and other deps actually live) must be allowed too.
        fs: { strict: true, allow: [join(repo, "frontend"), realpathSync(join(repo, "frontend", "node_modules"))], deny: [".env", ".env.*", "*.{crt,pem,key}", "**/.git/**", "**/.rumahl-dev/**", `${state}/**`] },
        watch: { usePolling: polling, interval: 250 }
      },
      plugins: [{
        name: "rumahl-ssr-reload",
        handleHotUpdate({ file, server }) {
          if (file.includes("/packages/ssr/src/")) server.ws.send({ type: "full-reload" });
        }
      }]
    });
    // Compile the initial SSR graph before accepting platform requests; the
    // production gateway keeps its bounded seven-second rendering deadline.
    await vite.ssrLoadModule(join(repo, "frontend/packages/ssr/src/render.tsx"));
    renderer = createRendererServer(async (payload) => {
      let module;
      try { module = await vite.ssrLoadModule(join(repo, "frontend/packages/ssr/src/render.tsx")); }
      catch (error) { console.error(`[ssr] ${error.message}`); throw error; }
      let rendered;
      try { rendered = await module.renderShellDocument(payload.snapshot, assets, payload.nonce, payload.requestPath); }
      catch (error) { console.error(`[ssr] ${error.message}`); throw error; }
      try {
        let html = "";
        for await (const chunk of rendered.stream) {
          html += chunk.toString();
          if (html.length > 1024 * 1024) throw new Error("render too large");
        }
        // Let Vite pre-transform the real entry, not the production-shaped alias.
        html = html.replaceAll(assets.script, "/src/main.tsx").replaceAll(assets.stylesheet, "/src/styles.css");
        html = (await vite.transformIndexHtml("/", html)).replaceAll(NONCE_PLACEHOLDER, payload.nonce);
        return { stream: Readable.from([html]), abort: rendered.abort };
      } catch (error) {
        console.error(`[ssr] ${error.message}`);
        rendered.abort();
        throw error;
      }
    }, { development: true });
    await new Promise((resolve, reject) => {
      renderer.once("error", reject);
      renderer.listen(join(runtime, "ssr.sock"), resolve);
    });
    await chmod(join(runtime, "ssr.sock"), 0o600);
    return { origin, reload: () => vite.ws.send({ type: "full-reload" }), close };
  } catch (error) {
    await close();
    throw error;
  }

  async function close() {
    await vite?.close();
    renderer?.closeAllConnections();
    renderer?.close();
    edge.closeAllConnections();
    edge.close();
  }
}
