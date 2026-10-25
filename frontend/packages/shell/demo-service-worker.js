/* Static demo service worker. It keeps only public demo assets and never
   intercepts API calls or other origins. VERSION is replaced at build time so
   every deployment gets a fresh cache and the normal waiting-worker flow. */
const VERSION = "__RUMAHL_DEMO_SW_VERSION__";
const CACHE = `rumahl-demo-${VERSION}`;
const CORE = ["/", "/index.html", "/manifest.webmanifest", "/icon.svg", "/icon-512.png"];

async function cacheShell(cache) {
  try {
    const response = await fetch("/", { cache: "reload" });
    if (!response.ok) return;
    const html = await response.clone().text();
    await cache.put("/", response.clone());
    await cache.put("/index.html", response.clone());
    const assets = [...html.matchAll(/(?:src|href)="(\/assets\/[^"]+)"/g)].map(match => match[1]);
    await Promise.all([...new Set(assets)].map(async path => {
      try {
        const asset = await fetch(path, { cache: "reload" });
        if (asset.ok) await cache.put(path, asset);
      } catch { /* partial offline cache is still useful */ }
    }));
  } catch { /* the network may be unavailable during install */ }
}

self.addEventListener("install", event => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    await Promise.all(CORE.map(async path => {
      try {
        const response = await fetch(path, { cache: "reload" });
        if (response.ok) await cache.put(path, response.clone());
      } catch { /* keep the old/partial cache */ }
    }));
    await cacheShell(cache);
  })());
});

self.addEventListener("activate", event => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith("rumahl-demo-") && key !== CACHE) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});

function offlineResponse() {
  return new Response("<!doctype html><meta name=viewport content='width=device-width,initial-scale=1'><title>rumahl</title><style>html,body{height:100%;margin:0}body{display:grid;place-items:center;background:#0b0f0d;color:#eef5f1;font:15px/1.6 system-ui,sans-serif;text-align:center}main{max-width:340px;padding:24px}p{color:#9aa79f}a{color:#beff2d}</style><main><h1>rumahl OS</h1><p>This page is not available offline yet. Reconnect and reload once to fill the offline cache.</p><p><a href='/'>Try again</a></p></main>", {
    status: 503,
    headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" }
  });
}

self.addEventListener("fetch", event => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin) return;
  if (url.pathname.startsWith("/api/") || url.pathname === "/service-worker.js") return;

  if (request.mode === "navigate") {
    event.respondWith((async () => {
      try {
        const response = await fetch(request);
        if (response.ok && (url.pathname === "/" || url.pathname === "/index.html" || url.pathname === "/demo.html")) {
          const cache = await caches.open(CACHE);
          await cache.put("/", response.clone());
          await cache.put("/index.html", response.clone());
        }
        return response;
      } catch {
        const cache = await caches.open(CACHE);
        return (await cache.match(request)) || (await cache.match("/index.html")) || (await cache.match("/")) || offlineResponse();
      }
    })());
    return;
  }

  if (!["script", "style", "image", "font", "manifest"].includes(request.destination)) return;
  event.respondWith((async () => {
    const cache = await caches.open(CACHE);
    const cached = await cache.match(request);
    if (cached) return cached;
    try {
      const response = await fetch(request);
      if (response.ok) await cache.put(request, response.clone());
      return response;
    } catch {
      return Response.error();
    }
  })());
});

self.addEventListener("message", event => {
  const data = event.data || {};
  if (data.type === "RUMAHL_SKIP_WAITING") {
    event.waitUntil(self.skipWaiting());
    return;
  }
  if (data.type === "RUMAHL_CONNECTIVITY" && event.ports?.[0]) {
    event.ports[0].postMessage({ connectivity: self.navigator?.onLine === false ? "device" : "server" });
    return;
  }
  if (data.type !== "RUMAHL_STATUS" || !event.ports?.[0]) return;
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    const resources = await Promise.all(CORE.map(path => cache.match(path)));
    event.ports[0].postMessage({ version: VERSION, cached: resources.filter(Boolean).length, total: CORE.length });
  })());
});
