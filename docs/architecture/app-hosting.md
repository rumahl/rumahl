# Installed web-app hosting

The shell resolves `/app/:appId/*` dynamically in desktop and launcher modes.
Trusted first-party apps register React routes. Installed apps receive an
isolated iframe; they never execute in the shell's JavaScript realm.

## Catalog and launch authority

`GET /api/v1/shell/apps` requires a live shell session and returns
`catalogVersion: 1` and bounded public entries (`id`, `installationId`, `title`,
`version`, `launchable`). Rust projects only installations authorized for the
current user by `rumahl.apps.launch`. Installation alone confers no access.
Explicit grants use namespace `rumahl.apps`, kind `installation`, and the
installation UUID as resource key. A system-scope grant permits all installations.
Grant issuance remains the trusted platform's responsibility; this slice adds
no browser API for issuing grants.

`POST /api/v1/shell/apps/:id/launch` requires the shell cookie, exact shell Origin
and JSON containing `installationId`, with optional `lease` for renewal. The
response (`launchVersion: 1`) binds app ID, installation ID, `frameUrl`, `lease`
and `renewAfterSeconds: 30`. Missing/unauthorized apps return 404; unavailable
storage/configuration returns 503. User data and leases are private and no-store.

Rust re-reads the live account, session, installation and grants for every
launch/renewal and asset access. An explicit grant for a removed installation
cannot authorize its replacement. The shell refreshes the catalog on snapshot
revisions and every 30 seconds, failing closed on errors. Open frames renew every
30 seconds; failure removes the frame and exposes Retry. Closing/minimizing a
window or leaving launcher content stops renewal. Already executing app code is
removed when the shell learns of revocation; revocation cannot erase content
already delivered to a browser.

## Isolation and asset transport

Each installation uses `<installation-uuid>.<RUMAHL_APP_HOST_SUFFIX>` on the
shell's HTTPS port. The default suffix is `apps.<shell-host>`. Asset paths are
`/launch/<256-bit-random-lease>/<manifest-entry-path>`. This narrow bearer
capability is bound to one session and installation, expires after 90 seconds
without renewal, and exists only in platform memory. Restart invalidates it;
the user can retry. Limits: 32 active leases per session, 1024 overall. A lease
never authorizes shell APIs or arbitrary files.

The iframe uses `sandbox="allow-scripts"` without `allow-same-origin`. A response
CSP also applies this sandbox on direct navigation. Both development and nginx
route app hosts only to the asset handler and strip cookies/Authorization. Rust
never routes an app host into the shell API. Referrers and access logs omit lease
URLs. Apps receive no shell session token or broad platform bearer credential.

CSP allows scripts/styles/images/fonts and connections only to the app's own
asset origin (plus inline scripts/styles and image data URLs), blocks child
frames, forms, objects and base URL changes, and limits ancestors to the shell.
Opaque-origin module requests receive `Access-Control-Allow-Origin: null`,
without credentials; a valid lease and fresh authorization are still required.
This is not a general platform API for `Origin: null`.

App deep paths and application query parameters become the entrypoint fragment
(`#/documents/42?view=detail`), with the shell's `mode` parameter removed. Assets
must use relative URLs. Navigation within the iframe does not yet synchronize
browser history back to the shell.

## Publication boundary and supported apps

Only `RuntimeKind::Web` with a manifest `main` web-asset entrypoint is launchable.
Assets must already be published under:

```text
$RUMAHL_STATE_DIR/app-assets/<installation-uuid>/<manifest-path>
```

Only the entrypoint's directory and descendants are served. This tree must
contain **public assets only**, never package secrets; an entrypoint at its root
makes the entire tree public. The publisher must stage immutable, verified assets
and activate them atomically. Directory/file reads use `openat` and `O_NOFOLLOW`,
reject nonregular files and traversal, allowlist MIME types, and limit individual
files to 2 MiB. Unpublished apps appear as not launchable in the authorized catalog.

Package verification, import and publication are implemented. `rumahl-app-packages`
verifies a signed directory package (detached Ed25519 signature over the exact
manifest bytes, plus SHA-256 per payload file) and `PackageImporter` publishes the
verified web assets under the installation root or stages the container image
reference through the runtime supervisor. The `rumahl-package` CLI generates
signing keys, signs packages and verifies them offline. An end-user
permission-management flow remains separate work. `app_host_fixture` is only a
disposable integration helper; it is not packaged into OS images. Container web
proxies, OIDC app login, networked app backends and an action/message bridge need
separate authorization contracts; this host does not silently grant them access.

## App <-> OS bridge, runtime control and channels

Installed apps are opaque-origin iframes and reach the shell only through
`window.postMessage`. The versioned `rumahl.bridge.v1` protocol (contract in
`@rumahl/contracts/bridge`) binds the channel to the frame's `contentWindow`
(`event.origin` is always `"null"` for a sandboxed app) and exposes capability
gated methods (`os.info`, `os.theme.get`, `os.notification`,
`os.window.{close,minimize,focus}`). The app's declared manifest permissions are
projected into the catalog and a method is refused unless its capability is
declared; the app side uses `@rumahl/bridge-client`.

Runtime control reaches the supervisor: `RuntimeController` and
`ProviderRuntimeAdapter` route `status`/`start`/`stop` over the
`AppRuntimeProvider` boundary, exposed as
`GET/POST /api/v1/shell/apps/{id}/runtime`. The catalog carries each app's
`lifecycle` (`always-on` | `on-demand`); on-demand container apps are prepared
but not activated at install.

Always-on services receive OS events without an open window over the persistent
`rumahl.channel.v1` channel: the app connects to a loopback endpoint, a
`hello`/token handshake authenticates the installation, and a live
`RuntimeChannel` is registered until it disconnects. The bridge and the channel
share the same method/event surface.

Connectors let an app ship a bundle for an external service (for example
Nextcloud or Plex): the manifest declares a connector target plus a
container-artifact entrypoint, and the OS installs the bundle and registers the
app as an OIDC relying party for it.

## Deployment and checks

Development uses `*.apps.localhost`, the developer HTTPS port, and a certificate
covering the wildcard and `localhost`. Older generated certificates are replaced
and need to be trusted again; custom certificates must cover both names.
Production nginx uses `*.apps.rumahl.home.arpa`. Local DNS must resolve those
hosts to the device, and trusted TLS must cover the wildcard and shell hostname.
Custom suffixes require matching nginx, DNS and TLS configuration. SSR receives
only the configured wildcard frame source, never leases or user credentials.

Tests cover catalog/launch authentication and CSRF, user filtering, expired
leases, logout, grant revocation, reinstall identity, symlink/traversal and size
denial. Chromium loads a module fixture through the Developer Server and checks
isolation, deep routes and revocation. The staged production HTTPS test exercises
the shipped nginx configuration, wildcard TLS and persistent sessions. A booted
Buildroot VM/device test is still required; process-level checks do not establish
image acceptance.
