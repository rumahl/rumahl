import { defineConfig } from "vitest/config";
import { readFileSync } from "node:fs";
import { shellBuildId } from "../shell/scripts/build-id.js";

const shellVersion = JSON.parse(readFileSync(new URL("../shell/package.json", import.meta.url), "utf8")).version as string;

export default defineConfig({
  define: {
    __RUMAHL_SHELL_BUILD_ID__: JSON.stringify(shellBuildId()),
    __RUMAHL_SHELL_VERSION__: JSON.stringify(shellVersion)
  },
  ssr: { noExternal: ["react", "react-dom", "scheduler", "react-router", "cookie", "set-cookie-parser"] },
  build: {
    sourcemap: true,
    target: "node22"
  },
  test: { environment: "node" }
});
