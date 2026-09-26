import { Buffer } from "node:buffer";
import { createServer } from "node:http";
import { URL } from "node:url";

// Both deployments use the same bounded snapshot-only renderer protocol.
export function createRendererServer(render, { development = false } = {}) {
  let inFlight = 0;

  return createServer(async (request, response) => {
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
      const { stream, abort } = await render(payload);
      response.writeHead(200, {
        "Cache-Control": "private, no-store",
        "Content-Security-Policy": `default-src 'none'; script-src 'self' 'nonce-${payload.nonce}'; style-src 'self'${development ? ` 'nonce-${payload.nonce}'` : ""}; img-src 'self' data:; connect-src 'self'; frame-src 'self'${frameOrigins.length ? ` ${frameOrigins.join(" ")}` : ""}; base-uri 'none'; object-src 'none'; form-action 'self'; frame-ancestors 'none'`,
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

}

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
