import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { test } from 'node:test';
import assert from 'node:assert/strict';
const source = readFileSync(new URL('../src/offline/service-worker.js', import.meta.url), 'utf8');
function worker(fetch) {
  const handlers = new Map(), entries = new Map(), deleted = [];
  const cache = { put: async (key, response) => entries.set(key, response), match: async key => entries.get(key)?.clone() };
  runInNewContext(source, {
    self: { location: { origin: 'https://os.test' }, clients: { claim: async () => {} }, addEventListener: (type, handler) => handlers.set(type, handler) },
    caches: { open: async () => cache, keys: async () => ['rumahl-os-offline-old', 'unrelated-app'], delete: async key => deleted.push(key) },
    URL, Response, Headers, fetch
  });
  return { entries, deleted, async message(data) { let task, result; handlers.get("message")({ data, ports: [{ postMessage: value => { result = value; } }], waitUntil: value => { task = value; } }); await task; return result; }, async lifecycle(name) { let task; handlers.get(name)({ waitUntil: value => { task = value; } }); await task; },
    request(path, overrides = {}) { let result; handlers.get('fetch')({ request: { url: `https://os.test${path}`, method: 'GET', mode: 'navigate', destination: 'document', ...overrides }, respondWith: value => { result = value; } }); return result; }
  };
}
test('precache contains only public, identity-independent resources fetched without credentials', async () => {
  const calls = [];
  const sw = worker(async (path, options) => { calls.push([path, options]); return new Response('public', { headers: { 'cache-control': 'public, max-age=0' } }); });
  await sw.lifecycle('install');
  assert.deepEqual([...sw.entries.keys()], ['/offline', '/offline/client.js', '/offline/wallpaper.jpg']);
  assert.ok(calls.every(([, options]) => options.credentials === 'omit' && options.redirect === 'error'));
  await sw.lifecycle('activate');
  assert.deepEqual(sw.deleted, ['rumahl-os-offline-old']);
});
test('no-store, private and unsuccessful responses cannot enter the cache', async () => {
  for (const policy of ['private', 'no-store', 'public, no-store']) {
    const sw = worker(async () => new Response('private', { headers: { 'cache-control': policy } }));
    await assert.rejects(sw.lifecycle('install'));
    assert.equal(sw.entries.size, 0);
  }
});
test('offline navigations use a generic 503 page; POST, API, frames and credentials are never cached or queued', async () => {
  const sw = worker(async () => { throw new TypeError('offline'); });
  sw.entries.set('/offline', new Response('public offline page', { headers: { 'content-type': 'text/html' } }));
  for (const [path, options] of [
    ['/login', { method: 'POST' }], ['/logout', { method: 'POST' }], ['/api/v1/shell/fs', {}], ['/oauth2/authorize', {}],
    ['/app/private', { destination: 'iframe' }], ['/login', { mode: 'cors', destination: '' }], ['/assets/private.js', { destination: 'script', mode: 'cors' }]
  ]) assert.equal(sw.request(path, options), undefined);
  for (const path of ['/login?user=alice', '/app/private']) {
    const response = await sw.request(path);
    assert.equal(response.status, 503);
    assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.equal(await response.text(), 'public offline page');
  }
  assert.deepEqual([...sw.entries.keys()], ['/offline']);
});
test('online session responses and authorization failures pass through unchanged without persistence', async () => {
  for (const status of [200, 401, 403, 503]) {
    const sw = worker(async () => new Response('private session response', { status, headers: { 'cache-control': 'private, no-store' } }));
    const response = await sw.request('/app/notes');
    assert.equal(response.status, status);
    assert.equal(await response.text(), 'private session response');
    assert.equal(sw.entries.size, 0);
  }
});

test('worker reports its own version and actual offline resource availability', async () => {
  const sw = worker(async () => new Response('public', { headers: { 'cache-control': 'public' } }));
  let status = await sw.message({ type: 'RUMAHL_STATUS' });
  assert.equal(status.version, '1.1.0');
  assert.equal(status.cached, 0);
  assert.equal(status.total, 3);
  await sw.lifecycle('install');
  status = await sw.message({ type: 'RUMAHL_STATUS' });
  assert.equal(status.cached, 3);
  assert.equal(await sw.message({ type: 'UNKNOWN' }), undefined);
});
