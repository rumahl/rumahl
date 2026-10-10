# Login transitions and offline support

The login progressively enhances the existing `/login` endpoint. A form POST
with `Accept: application/json` receives `{ "next": "/validated-shell-path" }`
only after successful password authentication and session creation. The same
Secure, HttpOnly cookie, origin checks and concurrency limit apply as to native
form submissions. Without JavaScript the existing redirect-based form remains
usable. Password failures return the server-rendered form; timeouts and network
failures are not retried automatically. The busy state prevents duplicate submits.

Supported browsers use View Transitions for login view changes and same-origin
navigation into the shell. Reduced-motion preferences and browsers without the
API retain working, non-animated navigation. Password values are cleared before
transition snapshots and when leaving the login document.

## Offline contract

`/os-client.js` registers `/service-worker.js` on the shell origin with scope `/`.
The worker caches exactly three public, account-independent resources:

- `/offline`
- `/offline/client.js`
- `/offline/wallpaper.jpg`

Installation fetches them without credentials, rejects redirects and rejects
private or no-store responses. There is no runtime caching of shell documents,
login HTML, API responses, profiles, sessions, files or app resources. POSTs and
other writes are never intercepted, queued or replayed. API, OAuth, recovery and
logout navigation, app frames and cross-origin requests retain their normal
network behavior. HTTP authorization errors pass through unchanged.

When a top-level GET navigation loses the network, the worker returns the generic
offline page with status 503. The page retries a public, non-cached connectivity
endpoint and restores the requested path when reachable. It cannot sign in or
open private apps offline. A successful prior online installation is required;
first-ever visits cannot work without a network connection.

Updates use the normal service-worker waiting lifecycle instead of forcing a
reload. Bump the cache version in `service-worker.js` when changing the public
offline kit. Activation removes only older `rumahl-os-offline-*` caches.
Private offline app data requires a separate, explicitly designed per-user
storage, authorization, expiry and synchronization model.

Cache isolation checks: `node --test platform-web/tests/service-worker.test.mjs`.
Login state tests are in `frontend/packages/shell/src/preferences/login-navigation.test.ts`;
the enhanced authentication contract is covered by `platform-service/tests/browser_flow.rs`.

## Device diagnostics

Settings → This device (`/settings/device`) displays the shell build, browser,
window/display mode, network hint, registration lifecycle, controlling worker,
worker version and actual cached resource count. The status message uses a
read-only MessageChannel exchange with the active worker. Update checks never
force a reload; a waiting update activates after all rumahl tabs are closed.
Browser fullscreen is not evidence of a managed kiosk configuration.

The normal shell entry also registers the worker, independently of SSR. Failed
registration is retryable from device settings; login registration errors are
reported in the browser console. Service workers require a secure context and a
trusted HTTPS certificate. Clicking through a certificate warning is insufficient
in Chromium: the page may load while worker registration fails with SecurityError.
Use the locally trusted certificate options documented in `../development.md`.
The frontend-only demo has no Rust offline endpoints and cannot install this worker.
