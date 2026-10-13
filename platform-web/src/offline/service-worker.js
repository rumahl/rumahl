/* Only this public, account-independent offline kit is ever persisted. */
const VERSION = '1.3.0';
const CACHE = 'rumahl-os-offline-v4';
// The offline page is fully self-contained, so it renders even when nothing
// else can be fetched. Only this public, account-independent kit is persisted.
const RESOURCES = ['/offline'];
// Neutral, well-known connectivity endpoints. They answer with an empty 204 and
// are the only external requests the offline kit ever makes. Replace or remove
// them to match a deployment's policy.
const CONNECTIVITY_PROBES = [
  'https://connectivitycheck.gstatic.com/generate_204',
  'https://cp.cloudflare.com/generate_204',
];

/**
 * Distinguishes "this device has no network" from "rumahl OS is unreachable but
 * this device is online". The page cannot make these probes (its CSP is
 * `connect-src 'self'`); the worker can.
 */
async function classifyConnectivity() {
  if (typeof self.navigator?.onLine === 'boolean' && self.navigator.onLine === false) return 'device';
  for (const url of CONNECTIVITY_PROBES) {
    try {
      await fetch(new Request(url, { mode: 'no-cors', cache: 'no-store', signal: AbortSignal.timeout(4000) }));
      return 'server';
    } catch { /* try the next probe */ }
  }
  return 'device';
}

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
  // Recovery and shell navigations fall back to the offline kit too, so a dead
  // device never shows the browser's own error page.
  if (/^\/(api|oauth2|logout)(\/|$)/.test(url.pathname)) return;
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
  const data = event.data || {};
  if (data.type === 'RUMAHL_CONNECTIVITY' && event.ports?.[0]) {
    event.waitUntil((async () => {
      const connectivity = await classifyConnectivity();
      event.ports[0].postMessage({ connectivity });
    })());
    return;
  }
  if (data.type !== 'RUMAHL_STATUS' || !event.ports?.[0]) return;
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    const resources = await Promise.all(RESOURCES.map(path => cache.match(path)));
    event.ports[0].postMessage({ version: VERSION, cached: resources.filter(Boolean).length, total: RESOURCES.length });
  })());
});
