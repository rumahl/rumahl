# App settings and brokered network access

Status: implemented for web apps in the developer and installed shell. This
document is the contract other plugins can rely on.

Third-party integration apps often need two things the sandbox deliberately
withholds: a place to keep configuration and a way to reach a local service.
Both are generic platform resources, not Home-Assistant features:

- `com.rumahl.os.settings` lets an app read and write **its own** manifest
  settings. Values are bound to the installation; secret values are
  write-only and encrypted at rest.
- `com.rumahl.os.network` lets an app ask the OS to perform an HTTP(S)
  request to an origin the user explicitly approved for that installation.
  The app never receives a socket, a cookie jar or the secret plaintext.

Neither resource is granted by installation alone. The signed manifest
declares the capability; the platform re-checks the live installation,
account and session on every call.

## Manifest

```json
{
  "permissions": [
    { "id": "com.rumahl.os.settings", "scope": "app-private", "required": true },
    { "id": "com.rumahl.os.network", "scope": "user-selected", "required": true }
  ],
  "settings": [
    { "key": "ha.url", "title": "Home Assistant address", "type": "text", "required": true },
    { "key": "ha.token", "title": "Access token", "type": "secret", "required": true },
    { "key": "ha.poll", "title": "Refresh interval", "type": "number" }
  ]
}
```

`secret` is a write-only setting kind. The manifest parser accepts it wherever
`text`, `number`, `boolean` or `select` are accepted. A secret must not declare
`options`; an empty secret clears the stored value.

## Settings values

Storage: `app_setting` in `platform.sqlite`, keyed by
`(installation_id, key)`. Plain values are JSON text validated against the
manifest kind. Secret values are sealed with the platform field cipher and an
associated context of `app-setting:<installation>:<key>`; a ciphertext moved to
another key or installation fails authentication.

If the host has a TPM, its sealed key encrypts setting secrets. Development and
TPM-less hosts use a random 32-byte key in
`<state>/keys/field.key` (mode `0600`). The key never enters SQLite, logs or a
browser bundle.

The shell API is:

- `GET /api/v1/shell/apps/{id}/settings?installationId=…` — manifest plus
  `values` (plain only) and `secrets` (configured secret keys only).
- `PUT /api/v1/shell/apps/{id}/settings?installationId=…` with
  `{ "values": { … } }` — validates keys and kinds, seals secrets and returns
  the same read shape.

The bridge methods are `os.settings.get` and `os.settings.set`. An app can only
read or write its own settings; it can never enumerate another installation.
Uninstall removes every value. Update keeps them because identity is stable.

Limits: 64 settings per manifest, 64 KiB per plain value, 16 KiB per secret,
≤ 64 settings in one write. A required value must be present after the write;
otherwise the request is denied.

## Network consent

An origin is `scheme://host[:port]` (no path, credentials, query or fragment).
Consent is per `(installation, user, origin)`:

- **session** — memory only, cleared when the browser session ends or the
  process restarts.
- **durable** — stored in `app_network_grant`, optionally with an expiry
  (maximum one year). Revocation is immediate on the next request.

The shell asks through `NetworkConsentDialog` when a request returns
`403 {"reason":"approval-required","origin":"…"}`. The user sees the exact
origin and the app title. The per-app settings page lists every active origin
and offers revocation. App uninstall removes all consents.

Durable grants honour the device policy `durable_grants_allowed`; if an
administrator caps consent at session scope, a durable request is refused.

## Brokered request

`POST /api/v1/shell/apps/{id}/network/request?installationId=…`

```json
{
  "url": "http://homeassistant.local:8123/api/states",
  "method": "GET",
  "headers": { "Accept": "application/json" },
  "body": "…optional text…",
  "secret": "ha.token",
  "maxBytes": 1048576
}
```

`secret` names one of the app's declared secret settings. The service reads and
decrypts it server-side and adds `Authorization: Bearer …`; the app cannot set
`Authorization`, `Cookie`, `Host` or `Origin`. The allow-list is `Accept`,
`Content-Type`, `Cache-Control`, `If-None-Match` and `If-Modified-Since`.

Response:

```json
{ "networkVersion": 1, "status": 200, "headers": { "content-type": "application/json" }, "body": "…", "truncated": false }
```

### Transport rules

- HTTP and HTTPS only; no URL credentials, no fragments.
- Methods `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`; bodies only on the
  mutating methods.
- No redirect following. A 30x is returned to the app as-is.
- The host is resolved once, forbidden destinations are rejected, and the
  chosen address is pinned for that request. Link-local, unspecified,
  broadcast and multicast targets are refused even if DNS resolves to them.
  Loopback and private ranges are allowed only through the user's explicit
  origin consent.
- 20 s total timeout, 10 s connect timeout, 256 KiB request body, 2 MiB
  response cap (clamped by `maxBytes`), 16 headers.
- Cookies are never sent. Response `Set-Cookie` is never exposed.

Consent changes and denied brokered requests are audited with the app and
installation, never the secret or the body. Successful polling is deliberately
not audited: a five-second dashboard would otherwise fill the activity log.
The OS returns `403` for a missing capability, a blocked target, missing
consent or an upstream failure; the app must not interpret these as success.

## Non-goals

- No WebSocket/SSE relay yet. Apps poll. A future `os.net.ws` can reuse the
  same consent and transport rules.
- No arbitrary headers, proxy configuration, client certificates or
  self-signed-certificate bypass.
- No direct app socket, no cookie/session sharing and no native bridge.
- No network access from an app that did not declare the capability, even if
  the user approved an origin earlier for another app.
