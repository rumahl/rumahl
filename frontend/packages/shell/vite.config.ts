import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { URL } from "node:url";
import webpackStatsPlugin from "rollup-plugin-webpack-stats";
import { shellBuildId } from "./scripts/build-id.js";

const splashSource = readFileSync(new URL("./splash.html", import.meta.url), "utf8");
const splashStyle = splashSource.match(/<style[^>]*>([\s\S]*?)<\/style>/)?.[1] ?? "";
const splashBody = splashSource.match(/<body[^>]*>([\s\S]*?)<\/body>/)?.[1] ?? "";
const splashMarkup = `<meta name="rumahl-shell" content="1" /><style>${splashStyle.replaceAll(' nonce="{{nonce}}"', "")}</style>`;
const splashBodyMarkup = splashBody.replaceAll(' nonce="{{nonce}}"', "");

/** Show the boot splash before the app bundle and styles have painted. */
const injectSplash = {
  name: "inject-splash",
  transformIndexHtml(html: string, ctx: { filename: string }) {
    if (ctx.filename.endsWith("demo-widget.html") || ctx.filename.endsWith("404.html")) return html;
    return html
      .replace("</head>", `${splashMarkup}</head>`)
      .replace(/<body([^>]*)>/, `<body$1>${splashBodyMarkup}`);
  }
};

const blockDemoEntry = {
  name: "block-demo-entry",
  configureServer(server: { middlewares: { use: (handler: Middleware) => void } }) {
    server.middlewares.use((request, response, next) => {
      if (request.url === "/demo.html" || request.url === "/demo-widget.html" || request.url?.startsWith("/src/demo/")) {
        response.statusCode = 404;
        response.end();
        return;
      }
      next();
    });
  }
};

/** Serves the static widget/app host for the local demo on its own origin. */
const demoWidgetHost = {
  name: "demo-widget-host",
  configureServer(server: { httpServer: { once: (event: string, callback: () => void) => void } | null }) {
    const html = readFileSync(new URL("./demo-widget.html", import.meta.url), "utf8");
    const widget = createServer((request, response) => {
      if (request.method !== "GET") {
        response.writeHead(404).end();
        return;
      }
      response.writeHead(200, {
        "Content-Type": "text/html; charset=utf-8",
        "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; base-uri 'none'; frame-ancestors http://localhost:* http://127.0.0.1:*",
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

const packageVersion = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version as string;

export default defineConfig(({ mode, command }) => {
  // In production the widget/app host is served from the demo origin itself
  // (paths /apps/* and /widgets/*). Local development keeps a second origin so
  // the sandbox behaves like the optional dedicated-host deployment.
  const demoWidgetOrigin = process.env.RUMAHL_DEMO_WIDGET_ORIGIN ??
    (command === "serve" ? "http://127.0.0.1:5174" : "");

  return {
    define: {
      __RUMAHL_SHELL_BUILD_ID__: JSON.stringify(shellBuildId()),
      __RUMAHL_SHELL_VERSION__: JSON.stringify(packageVersion),
      ...(mode === "demo" ? { __RUMAHL_DEMO_WIDGET_ORIGIN__: JSON.stringify(demoWidgetOrigin) } : {})
    },

    plugins: [
      react(),
      injectSplash,
      ...(mode === "demo" ? [demoWidgetHost] : [blockDemoEntry]),
      ...(command === "build" && mode !== "demo" && mode !== "demo-widget" ? [webpackStatsPlugin()] : [])
    ],

    build: {
      manifest: true,
      sourcemap: true,
      target: "es2022",
      // Lightning CSS collapses the -webkit-/standard backdrop-filter pair and
      // drops the standard property, which removes the blur in Chromium builds.
      cssMinify: "esbuild",
      ...(mode === "demo" && command === "build" ? {
        outDir: "dist-demo",
        rollupOptions: {
          input: {
            demo: new URL("./demo.html", import.meta.url).pathname,
            widget: new URL("./demo-widget.html", import.meta.url).pathname,
            notFound: new URL("./404.html", import.meta.url).pathname
          }
        }
      } : {}),
      ...(mode === "demo-widget" && command === "build" ? {
        outDir: "dist-demo-widget",
        rollupOptions: { input: new URL("./demo-widget.html", import.meta.url).pathname }
      } : {})
    },

    test: {
      environment: "jsdom",
      setupFiles: "./src/test/setup.ts"
    }
  };
});
