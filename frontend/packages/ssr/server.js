import { chmodSync, lstatSync, readFileSync } from "node:fs";
import { isAbsolute, dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { createRendererServer } from "./renderer-http.js";

const packageRoot = dirname(fileURLToPath(import.meta.url));
const socketPath = process.env.RUMAHL_SSR_SOCKET;
const socketAccess = process.env.RUMAHL_SSR_SOCKET_ACCESS ?? "owner";
if (!socketPath || !isAbsolute(socketPath)) {
  throw new Error("RUMAHL_SSR_SOCKET must be an absolute private socket path");
}
if (socketAccess !== "owner" && socketAccess !== "group") {
  throw new Error("RUMAHL_SSR_SOCKET_ACCESS must be owner or group");
}
const parent = lstatSync(dirname(socketPath));
if (!parent.isDirectory() || parent.isSymbolicLink() || (parent.mode & 0o027) !== 0 ||
    (socketAccess === "group" && (parent.mode & 0o010) === 0)) {
  throw new Error("SSR socket directory must be private");
}
process.umask(0o077);

const manifest = JSON.parse(
  readFileSync(join(packageRoot, "../shell/dist/.vite/manifest.json"), "utf8")
);
const entry = manifest["index.html"];
if (!entry?.file || !entry.css?.[0]) {
  throw new Error("matching client assets are missing");
}
const assets = { script: `/${entry.file}`, stylesheet: `/${entry.css[0]}` };
const { renderShellDocument, SSR_SHELL_BUILD_ID } = await import("./dist/render.js");
const clientBuild = JSON.parse(
  readFileSync(join(packageRoot, "../shell/dist/build-id.json"), "utf8")
);
if (clientBuild.shellBuildId !== SSR_SHELL_BUILD_ID) {
  throw new Error("SSR and client assets belong to different shell builds");
}
const server = createRendererServer((payload) =>
  renderShellDocument(payload.snapshot, assets, payload.nonce)
);

server.listen(socketPath, () => {
  const socket = lstatSync(socketPath);
  if (socketAccess === "group" && socket.gid !== parent.gid) {
    server.close();
    throw new Error("SSR socket group does not match its private directory");
  }
  chmodSync(socketPath, socketAccess === "group" ? 0o660 : 0o600);
});
