# rumahl OpenID Provider domain

`rumahl-oidc-provider` owns transport-neutral OpenID Connect client
registration rules. It does not expose HTTP endpoints and does not contain
app-specific behavior.

Only apps already present in `PlatformState` can be registered. Their signed
manifest declares a logical runtime entrypoint, callback path, client type,
and requested OpenID scopes. A platform-owned origin resolver supplies the
actual HTTPS origin, so an app cannot register an arbitrary host.

Static web apps are public clients and receive no secret. Container apps are
confidential clients; a random 256-bit secret is returned once to the runtime
secret channel while only its SHA-256 digest is persisted. All registrations
require Authorization Code with PKCE `S256`, and redirect matching is exact.

Native clients remain outside this first slice because native runtime
installation is not yet enabled by the core trust policy. Their claimed HTTPS,
private-use scheme, or loopback callback handling must be added together with
that policy rather than treated as a web redirect exception.

`OidcAppLifecycle` is the integration boundary for installing and uninstalling
apps. It runs the core lifecycle against cloned platform state and grants,
performs the atomic OIDC repository write, and publishes the staged live state
only after that write succeeds. A repository failure therefore cannot leave a
half-installed app in the running OS. Durable crash atomicity between the
platform snapshot and OIDC tables remains a persistence-layer transaction for
the Buildroot runtime integration.
