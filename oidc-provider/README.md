# rumahl OpenID Provider domain

`rumahl-oidc-provider` owns transport-neutral OpenID Connect registration,
authorization, token and signing rules. The optional HTTP adapter lives in
`rumahl-platform-web`; this crate contains no app-specific behavior.

Only apps already present in `PlatformState` can be registered. Their signed
manifest declares a logical runtime entrypoint, callback path, client type,
and requested OpenID scopes. A platform-owned origin resolver supplies the
actual HTTPS origin, so an app cannot register an arbitrary host.

Static web apps are public clients and receive no secret. Container apps are
confidential clients; a random 256-bit secret is encrypted through the core
`SecretStore`, while only its SHA-256 digest is persisted with the OIDC client.
Interrupted registration can return the same client and secret after verifying
the complete declaration-derived registration. All registrations require
Authorization Code with PKCE `S256`, and redirect matching is exact.

Native clients remain outside this first slice because native runtime
installation is not yet enabled by the core trust policy. Their claimed HTTPS,
private-use scheme, or loopback callback handling must be added together with
that policy rather than treated as a web redirect exception.

The recovery-safe registrar is the only registration path. It is the OIDC
participant of the general app-operation journal; the older in-process
`OidcAppLifecycle` and non-recoverable registrar entry point were removed.
`rumahl-app-operations` persists participant transitions, delivers a recovered
confidential secret through an idempotent runtime channel, and publishes the
staged platform state only after snapshot and journal commit.

Authorization transactions, user consent, one-use code digests, pairwise
subjects, opaque access grants and refresh-token families have separate domain
contracts and SQLite stores. The first live protocol slice supports
Authorization Code with mandatory PKCE `S256`, EdDSA-signed ID tokens,
discovery/JWKS and UserInfo. It deliberately rejects `offline_access` until
the HTTP refresh grant can issue and rotate token families atomically. The
signing seed is supplied by an OS-owned provider; startup integration and key
rotation are still pending. The real Nextcloud container acceptance sequence
is described in `tests/e2e/nextcloud` and remains to be executed.
