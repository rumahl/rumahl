import { defineConfig } from "vitest/config";
import { shellBuildId } from "../shell/scripts/build-id.js";

export default defineConfig({
  define: { __RUMAHL_SHELL_BUILD_ID__: JSON.stringify(shellBuildId()) },
  ssr: { noExternal: ["react", "react-dom", "scheduler", "react-router", "cookie", "set-cookie-parser"] },
  build: {
    sourcemap: true,
    target: "node22"
  },
  test: { environment: "node" }
});
