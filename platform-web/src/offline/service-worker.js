/* Only this public, account-independent offline kit is ever persisted. */
const VERSION = '1.1.0';
const CACHE = 'rumahl-os-offline-v2';
const RESOURCES = ['/offline', '/offline/client.js', '/offline/wallpaper.jpg'];
self.addEventListener('install', event => {
  event.waitUntil((async () => {
    const responses = await Promise.all(RESOURCES.map(async path => {
      const response = await fetch(path, { credentials: 'omit', cache: 'reload', redirect: 'error' });
      const policy = response.headers.get('cache-control') || '';
      if (!response.ok || !/\bpublic\b/i.test(policy) || /private|no-store/i.test(policy)) throw new Error('Offline resource must be public');
      return [path, response];
    }));
    const cache = await caches.open(CACHE);
    await Promise.all(responses.map(([path, response]) => cache.put(path, response)));
  })());
});
self.addEventListener('activate', event => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith('rumahl-os-offline-') && key !== CACHE) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});
self.addEventListener('fetch', event => {
  const request = event.request;
  const url = new URL(request.url);
  // No POST interception, offline writes, credentials cache, or app iframe fallback.
  if (request.method !== 'GET' || url.origin !== self.location.origin) return;
  if (!url.search && RESOURCES.includes(url.pathname)) {
    event.respondWith((async () => (await (await caches.open(CACHE)).match(url.pathname)) || fetch(request))());
    return;
  }
  if (request.mode !== 'navigate' || request.destination !== 'document') return;
  if (/^\/(api|oauth2|shell|recovery|logout)(\/|$)/.test(url.pathname)) return;
  event.respondWith((async () => {
    try { return await fetch(request); }
    catch {
      const offline = await (await caches.open(CACHE)).match('/offline');
      if (!offline) return Response.error();
      const headers = new Headers(offline.headers);
      headers.set('cache-control', 'no-store');
      return new Response(await offline.blob(), { status: 503, statusText: 'Offline', headers });
    }
  })());
});

// Read-only diagnostics: never include account or session data.
self.addEventListener('message', event => {
  if (event.data?.type !== 'RUMAHL_STATUS' || !event.ports?.[0]) return;
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    const resources = await Promise.all(RESOURCES.map(path => cache.match(path)));
    event.ports[0].postMessage({ version: VERSION, cached: resources.filter(Boolean).length, total: RESOURCES.length });
  })());
});
