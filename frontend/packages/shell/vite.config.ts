import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { URL } from "node:url";
import { shellBuildId } from "./scripts/build-id.js";

const blockDemoEntry = {
  name: "block-demo-entry",
  configureServer(server: { middlewares: { use: (handler: Middleware) => void } }) {
    server.middlewares.use((request, response, next) => {
      if (request.url === "/demo.html" || request.url?.startsWith("/src/demo/")) {
        response.statusCode = 404;
        response.end();
        return;
      }
      next();
    });
  }
};

const demoWidgetHost = {
  name: "demo-widget-host",
  configureServer(server: { httpServer: { once: (event: string, callback: () => void) => void } | null }) {
    const html = readFileSync(new URL("./src/demo/widget.html", import.meta.url), "utf8");
    const widget = createServer((request, response) => {
      if (request.method !== "GET" || request.url !== "/") {
        response.writeHead(404).end();
        return;
      }
      response.writeHead(200, {
        "Content-Type": "text/html; charset=utf-8",
        "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; frame-ancestors http://localhost:5173",
        "Cache-Control": "no-store"
      });
      response.end(html);
    });
    widget.listen(5174, "127.0.0.1");
    server.httpServer?.once("close", () => widget.close());
  }
};

type Middleware = (
  request: { url?: string },
  response: { end: () => void; statusCode: number },
  next: () => void
) => void;

export default defineConfig(({ mode }) => ({
  define: { __RUMAHL_SHELL_BUILD_ID__: JSON.stringify(shellBuildId()) },
  plugins: [react(), ...(mode === "demo" ? [demoWidgetHost] : [blockDemoEntry])],
  build: {
    manifest: true,
    sourcemap: true,
    target: "es2022"
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts"
  }
}));
