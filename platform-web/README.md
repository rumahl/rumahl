# Local shell gateway

`rumahl-platform-web` is a local Unix-socket HTTP adapter, not the system's
public HTTPS server. The HTTPS edge must preserve the browser's `Cookie`,
`Origin`, and `Host` headers, route `/`, `/recovery`, and `/api/v1/shell/*`
to this socket, and serve the matching immutable `/assets/*` client bundle
and authorized `/shell/themes/*` stylesheets. It must set the
host-only, Secure, HttpOnly, SameSite session cookie after login. No identity
header from the edge is trusted by this adapter.

The caller supplies the existing `LocalSessionAuthenticator` (through
`PlatformShellBackend`), a `ShellSnapshotProvider`, a `ShellEventSource`, and a
`WidgetFrameResolver`. The widget resolver is responsible for app-specific
authorization and routing. The gateway additionally checks that the widget is
present in the authenticated snapshot and that its frame URL is on a distinct
HTTPS host. The in-memory event bus is useful until a durable platform event
feed is wired in; it does not depend on the app-container engine.

`/` sends only a validated snapshot, a fresh nonce, and authorized frame
origins to the private SSR socket. Cookies and identity headers are not
forwarded. If SSR fails, the route returns 503; `/recovery` remains a static,
asset-free Rust response in English or German. It intentionally offers no
privileged action yet. Recovery actions require a separate, audited local
administrator authorization and last-known-good store before they can safely
be exposed.

Buildroot service wiring, public TLS termination, login-cookie issuance,
immutable asset serving, durable event publication, and privileged recovery
operations are deployment/integration work still to be done. The library does
not silently provide mock data in production.
