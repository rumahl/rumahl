import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

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

type Middleware = (
  request: { url?: string },
  response: { end: () => void; statusCode: number },
  next: () => void
) => void;

export default defineConfig(({ mode }) => ({
  plugins: [react(), ...(mode === "demo" ? [] : [blockDemoEntry])],
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
