import { chmodSync, lstatSync, readFileSync } from "node:fs";
import { Buffer } from "node:buffer";
import { createServer } from "node:http";
import { isAbsolute, dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath, URL } from "node:url";

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
let inFlight = 0;

const server = createServer(async (request, response) => {
  if (request.method !== "POST" || request.url !== "/render") {
    reply(response, 404);
    return;
  }
  if (request.headers.cookie || request.headers["content-type"] !== "application/json") {
    reply(response, 400);
    return;
  }
  request.setTimeout(5_000, () => request.destroy());
  if (inFlight >= 16) {
    reply(response, 503);
    return;
  }
  inFlight += 1;
  let released = false;
  const release = () => {
    if (released) return;
    released = true;
    inFlight -= 1;
  };
  response.once("close", release);

  try {
    const payload = JSON.parse(await readBoundedBody(request));
    const keys = payload && typeof payload === "object" ? Object.keys(payload).sort().join(",") : "";
    if (keys !== "nonce,snapshot" && keys !== "frameOrigins,nonce,snapshot") {
      reply(response, 400);
      return;
    }
    const frameOrigins = validFrameOrigins(payload.frameOrigins ?? []);
    const { stream, abort } = await renderShellDocument(payload.snapshot, assets, payload.nonce);
    response.writeHead(200, {
      "Cache-Control": "private, no-store",
      "Content-Security-Policy": `default-src 'none'; script-src 'self' 'nonce-${payload.nonce}'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-src 'self'${frameOrigins.length ? ` ${frameOrigins.join(" ")}` : ""}; base-uri 'none'; object-src 'none'; frame-ancestors 'none'`,
      "Content-Type": "text/html; charset=utf-8",
      "X-Content-Type-Options": "nosniff"
    });
    response.on("close", abort);
    stream.on("error", () => response.destroy());
    stream.pipe(response);
  } catch (error) {
    release();
    if (!response.headersSent) {
      const status = error?.message === "request too large" ? 413 :
        error?.message?.startsWith("invalid frame origin") ? 400 : 503;
      reply(response, status);
    }
    else response.destroy();
  }
});

server.listen(socketPath, () => {
  const socket = lstatSync(socketPath);
  if (socketAccess === "group" && socket.gid !== parent.gid) {
    server.close();
    throw new Error("SSR socket group does not match its private directory");
  }
  chmodSync(socketPath, socketAccess === "group" ? 0o660 : 0o600);
});

function reply(response, status) {
  response.writeHead(status, {
    "Cache-Control": "private, no-store",
    "Content-Type": "text/plain; charset=utf-8",
    "X-Content-Type-Options": "nosniff"
  });
  response.end();
}

async function readBoundedBody(request) {
  const chunks = [];
  let length = 0;
  for await (const chunk of request) {
    length += chunk.length;
    if (length > 300 * 1024) throw new Error("request too large");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

function validFrameOrigins(value) {
  if (!Array.isArray(value) || value.length > 32) {
    throw new Error("invalid frame origin list");
  }
  return value.map((candidate) => {
    if (typeof candidate !== "string" || candidate.length > 256) {
      throw new Error("invalid frame origin");
    }
    let url;
    try {
      url = new URL(candidate);
    } catch {
      throw new Error("invalid frame origin");
    }
    if (url.protocol !== "https:" || url.origin !== candidate || url.username || url.password) {
      throw new Error("invalid frame origin");
    }
    return url.origin;
  });
}
