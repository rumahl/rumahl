# rumahl OpenID Provider domain

`rumahl-oidc-provider` owns transport-neutral OpenID Connect client
registration rules. It does not expose HTTP endpoints and does not contain
app-specific behavior.

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

`OidcAppLifecycle` remains the in-process compatibility boundary for installing
and uninstalling apps. The recovery-safe registrar is the OIDC participant for
the general app-operation journal. `rumahl-app-operations` now provides the
top-level restart-safe installation coordinator, persists every participant
transition, delivers a recovered confidential secret through an idempotent
runtime channel, and publishes the staged platform state only after the final
snapshot and journal commit.
