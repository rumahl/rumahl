# Local shell gateway

`rumahl-platform-web` is a local Unix-socket HTTP adapter, not the system's
public HTTPS server. The HTTPS edge must preserve the browser's `Cookie`,
`Origin`, and `Host` headers, route `/`, `/recovery`, `/api/v1/shell/*`,
`/.well-known/openid-configuration`, `/oauth2/*`, `/login`, `/logout`, and
`/shell/themes/*` to this socket, and serve the matching immutable `/assets/*`
client bundle. The optional `BrowserSessions` adapter issues the
host-only, Secure, HttpOnly, SameSite session cookie after login. No identity
header from the edge is trusted by this adapter.

The caller supplies the existing `LocalSessionAuthenticator` (through
`PlatformShellBackend`), a `ShellSnapshotProvider`, a `ShellEventSource`, and a
`WidgetFrameResolver`. The widget resolver is responsible for app-specific
authorization and routing. The gateway additionally checks that the widget is
present in the authenticated snapshot and that its frame URL is on a distinct
HTTPS host. The in-memory event bus is useful until a durable platform event
feed is wired in; it does not depend on the app-container engine.

OIDC is optional in `GatewayState`. When configured, the gateway exposes
discovery, JWKS, an English/German consent page, Authorization Code with
mandatory PKCE `S256`, a token endpoint and UserInfo. Consent POSTs require
the exact public Origin and live OS session cookie. The protocol uses the
durable client, authorization, access-token and account repositories; disabling
OIDC does not disable the Shell or recovery page. Refresh grants, a privileged
logout/revocation transport and signing-key rotation are not exposed yet.

`/` sends only a validated snapshot, a fresh nonce, and authorized frame
origins to the private SSR socket. Cookies and identity headers are not
forwarded. If SSR fails, the route returns 503; `/recovery` remains a static,
asset-free Rust response in English or German. It intentionally offers no
privileged action yet. Recovery actions require a separate, audited local
administrator authorization and last-known-good store before they can safely
be exposed.

`platform-service` now composes the SQLite-backed browser login, logout,
snapshots and stock theme. `platform-buildroot` supplies separate service units,
an nginx HTTPS edge, artifact staging and a QEMU external tree. Its complete
HTTPS process test lives in `tests/e2e/shell`; a full image boot/reboot remains a
separate acceptance step. Privileged recovery actions and a journal-backed event
feed remain future work. The current service polls persisted state and uses
in-memory notifications; reconnecting clients always reload a snapshot.
