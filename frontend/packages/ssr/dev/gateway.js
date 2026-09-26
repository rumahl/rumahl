import console from "node:console";
import { createHash } from "node:crypto";
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

export async function startGateway({ repo, runtime, state, cert, key, port, buildId, polling }) {
  const platformSocket = join(runtime, "gateway.sock");
  let vite;
  let origin;
  const edge = createHttpsServer({ cert: await readFile(cert), key: await readFile(key) }, (request, response) => {
    if (request.headers.host !== new URL(origin).host) {
      response.writeHead(421).end();
      return;
    }
    // Vite serves source code, so never permit cross-origin browser access.
    if (request.headers.origin && request.headers.origin !== origin) {
      response.writeHead(403).end();
      return;
    }
    const path = request.url.split("?")[0];
    if (frontendPath.test(path)) {
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
      if (response.headersSent) response.destroy();
      else response.writeHead(503, { "Cache-Control": "no-store", "Content-Type": "text/plain" }).end("Platform restarting. Please retry shortly.");
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
        fs: { strict: true, allow: [join(repo, "frontend")], deny: [".env", ".env.*", "*.{crt,pem,key}", "**/.git/**", "**/.rumahl-dev/**", `${state}/**`] },
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
      try { rendered = await module.renderShellDocument(payload.snapshot, assets, payload.nonce); }
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
