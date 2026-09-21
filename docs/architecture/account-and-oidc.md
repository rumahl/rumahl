# Local accounts and the rumahl OpenID Provider

## Goal

rumahl OS supports multiple local accounts on one device. A user can sign in
to an installed app with a rumahl OS account through the OpenID Connect
protocol. A locally installed Nextcloud instance is a representative relying
party.

rumahl owns the account, session, consent, and client-registration systems. It
does not invent a proprietary authentication protocol or cryptographic format.
The provider implements OpenID Connect on OAuth 2.0 and follows the current
OAuth security best current practice.

## Terms and boundaries

- `LocalAccount` belongs to the rumahl OS identity plane. It is not a web
  account, an app-owned user, or a mandatory rumahl cloud identity.
- Multi-user isolation is an OS responsibility: sessions, home/profile data,
  permissions, background work, audit attribution, and app authorizations stay
  attached to the originating `UserId`.
- A `UserIdentity` is the stable local platform principal.
- A local account contains login, profile, status, and recovery metadata for
  one `UserIdentity`.
- A desktop session records which account is currently active in the OS shell.
- An OIDC client session belongs to one account and one installed client.
- An installed app is not an account and cannot issue identity tokens.
- Linking a future rumahl cloud identity must remain optional. Local sign-in
  and local OIDC continue to work without Internet access.

The OpenID Provider is likewise an OS platform service rather than a
Nextcloud-specific bridge. Any installed app can become a relying party through
the same install-time client-registration policy. Nextcloud is one required
interoperability target, not a special identity authority or architectural
dependency.

Account profile data, credentials, sessions, OAuth grants, tokens, and signing
keys are not app-manifest fields. Password hashes, refresh tokens, client
secrets, recovery material, and private signing keys are also excluded from the
general `PlatformSnapshot`.

## Account model

The first account domain slice adds these concepts without changing the
existing meaning of `UserId`:

```text
LocalAccount
├── user_id: UserId
├── username
├── display_name
├── status: active | locked | disabled
└── profile metadata

AccountCredential
├── user_id
├── credential kind
├── verifier metadata
└── lifecycle timestamps

AccountSession
├── session_id: SessionId
├── user_id
├── authentication time
├── last-seen time
├── reauthentication time
└── expiry/revocation state
```

Credential implementations are adapters. Password verifiers, passkey private
material, TOTP secrets, recovery codes, and equivalent secrets do not enter the
domain model as plain strings.

The first login adapter uses Argon2id password verifiers with a unique random
salt and PHC-encoded parameters. Password enrollment normalizes Unicode to NFC,
accepts spaces and Unicode without composition rules, requires at least 15
characters for the single-factor flow, and requires an offline blocklist
implementation. Failed attempts and temporary lockout state are persisted per
account. Unknown users, wrong passwords, missing credentials, locked accounts,
and throttled accounts return the same public authentication failure.

Password is not the account model. Passkeys and future device-backed methods
can establish the same verified local-account result and enter the identical
OS-session issuance path without changing `LocalAccount` or OIDC semantics.

Creating a password-backed account is one persistence transaction: the active
local account and its verifier either both become durable or neither does. A
password change consumes a fresh, non-copyable authentication result (five
minutes by default), replaces the verifier, and revokes every live OS session
and its opaque transport credentials in the same transaction. The in-memory
account state is committed only after durable persistence succeeds; a storage
or revocation failure leaves the previous password and sessions intact.

Multiple accounts can have live sessions at the same time. Switching the
desktop account selects another session; it does not reassign existing app
tokens, grants, background jobs, or audit entries to that account.

An internal `SessionId` identifies a session but is not itself a bearer secret.
HTTP cookies, IPC peer credentials, device-bound tokens, and future native
transports resolve their own opaque credentials to a session at the transport
boundary. Every request then revalidates the current OS account status, session
expiry, and revocation state before creating an `OperationContext`.

## OpenID Provider

The OS exposes a stable HTTPS issuer and at least these protocol surfaces:

```text
/.well-known/openid-configuration
/oauth2/authorize
/oauth2/token
/oauth2/userinfo
/oauth2/revoke
/oauth2/jwks
/oauth2/logout
```

The discovery document publishes the exact issuer, authorization endpoint,
token endpoint, supported code challenge methods, signing algorithms, claims,
scopes, and JWKS location.

The initial human sign-in flow is Authorization Code with PKCE `S256`:

- no implicit flow;
- no resource-owner password grant;
- exact redirect-URI matching;
- short-lived, one-time authorization codes;
- transaction-bound `state`, `nonce`, and PKCE values;
- audience-restricted access tokens;
- rotating refresh-token families with replay detection;
- asymmetric ID-token signing and key rotation;
- explicit expiry and revocation for every session and token family.

ID tokens are signed JWTs because this is required for interoperable OpenID
Connect relying parties. Access tokens are opaque initially and are resolved by
the provider for `userinfo` and protected platform APIs. This avoids exposing
unnecessary account data or coupling apps to an internal token schema.

The required initial scopes are:

```text
openid
profile
```

`email` is exposed only when the selected local account actually has a verified
address. `offline_access` requires explicit consent and creates a revocable,
rotating refresh-token family. Platform permissions such as file access remain
rumahl permission grants and are not silently implied by OIDC profile scopes.

## Privacy between installed apps

OIDC `sub` is not the raw `UserId`. The provider issues a stable pairwise
subject identifier for the account and the registered client sector. This
prevents unrelated installed apps from correlating the same local account.

The subject mapping is persisted so backup and recovery do not silently create
a different identity inside an existing app. A client never receives another
account's subject, consent, refresh-token family, or profile claims.

## Client registration during app installation

OIDC registration is part of the trusted app lifecycle. An app declares the
capability in its manifest, but rumahl generates the concrete `client_id`,
redirect URI, and optional client credential for the particular installation.

The manifest declares logical callback targets rather than arbitrary host
addresses:

```yaml
authentication:
  oidc:
    clientType: confidential
    callback:
      endpoint: web
      path: /apps/oidc/callback
    scopes:
      - openid
      - profile
```

The platform resolves the endpoint through the installed runtime descriptor.
The manifest cannot choose an external issuer, arbitrary host port, wildcard
redirect, or host filesystem path.

The first implementation persists the logical declaration as part of the app
snapshot and creates the concrete registration in a dedicated OIDC repository.
The declaration is optional: an app without it receives no OIDC client and the
client repository is not touched. Other optional resources, including managed
app databases, do not implicitly enable OIDC.
Only an `InstallationId` currently present in `PlatformState` can be
registered. Static web runtimes must declare a public client; container
runtimes must declare a confidential client whose callback references a
declared runtime endpoint. The platform origin resolver supplies the HTTPS
origin, while the manifest supplies only a strictly parsed absolute path.

Each installation receives a random 192-bit `client_id`. Confidential clients
also receive a random 256-bit secret exactly once for delivery through the
runtime secret channel; SQLite stores only its SHA-256 digest. Public clients
receive no secret. Both client types require Authorization Code with PKCE
`S256`, and authorization requests are accepted only when the supplied redirect
URI is byte-for-byte equal to the canonical registered URI. Native callback
exceptions are deferred until the native runtime trust policy exists.

`OidcAppLifecycle` is the current integration boundary above the core
`AppLifecycle`. Installation and uninstallation first run against cloned
platform state and grants. The live state is replaced only after the atomic
OIDC repository insert or revocation succeeds, so repository failures roll the
in-process lifecycle back without publishing partial state. Durable crash
consistency between the platform snapshot, OIDC metadata, and external app
resources is provided by the operation journal plus idempotent startup
reconciliation. A shared SQLite transaction may optimize metadata stored in
one database, but is not assumed across provider boundaries.

The general `AppOperation` journal records OIDC as an optional participant
alongside app databases and the final platform snapshot. It does not make OIDC
a prerequisite for installation. `AppOperationRunner` is now the restart-safe
top-level installation boundary: it persists the validated install target,
replays the OIDC registrar after interruption, acknowledges confidential
runtime-secret delivery through the same journal step, and publishes live
platform state only after the final snapshot and commit. `OidcAppLifecycle`
remains an in-process compatibility boundary for callers not yet migrated to
the operation runner.

- Container and server-side web apps such as Nextcloud are confidential
  clients. Their generated secret is injected through the runtime secret
  channel and is never written into the manifest or React bundle.
- Static React apps are public clients. They receive no client secret and must
  use Authorization Code with PKCE `S256`.
- Native clients use the system browser and a platform-owned callback or a
  tightly registered loopback redirect. Embedded credential webviews are not
  accepted.

Uninstalling an app revokes its authorization codes, consents, refresh-token
families, client sessions, and client credentials atomically with the app
installation. Reinstalling creates a new client registration unless a trusted
restore preserves the original installation identity and OIDC state.

## Nextcloud example

```text
1. rumahl installs the Nextcloud container.
2. AppLifecycle registers its logical OIDC callback.
3. The provider creates a confidential client bound to the InstallationId.
4. The runtime receives the client secret through a secret channel.
5. Nextcloud redirects the browser to the rumahl authorization endpoint.
6. The OS account chooser selects one local account.
7. rumahl shows requested profile scopes and consent.
8. Nextcloud exchanges the one-time code with PKCE and client authentication.
9. Nextcloud validates issuer, audience, nonce, signature, and expiry.
10. Nextcloud maps the pairwise subject to its local user record.
```

Selecting another OS account starts a new authorization transaction and yields
a different pairwise subject. It never mutates the first Nextcloud user's
session.

## Persistence and secret storage

P3 is extended with dedicated repositories instead of adding secrets to the
general platform snapshot:

```text
AccountRepository
AccountSessionRepository
OidcClientRepository
OidcConsentRepository
AuthorizationCodeRepository
TokenFamilyRepository
PairwiseSubjectRepository
SigningKeyStore
SecretStore
```

SQLite can implement the local metadata repositories transactionally. Secret
values are encrypted through `SecretStore`; the Buildroot integration decides
whether the root key comes from TPM-backed storage, a device key, or another
platform facility. Databases contain hashes for authorization codes, refresh
tokens, recovery codes, and confidential client secrets rather than reusable
plain values.

The first `SecretStore` implementation now encrypts installation-bound values
with AES-256-GCM, authenticates their owner/purpose metadata, and resolves root
keys through a separate key-provider contract. SQLite never contains the root
key or reusable plaintext. Buildroot still needs to supply the production
device-bound key provider.

`OidcClientRegistrar::register_or_recover_installed_app` now stores a
confidential client secret before inserting its digest-only client record. A
restart verifies and returns the same active client and decrypted secret;
missing secrets or declaration mismatches fail closed. Public clients never
create secret material. The operation runner now persists successful runtime
delivery by completing the OIDC journal step; the production Buildroot runtime
channel remains to be implemented.

## Delivery phases

### P3 extension — account persistence

- local account registry and uniqueness rules;
- account/session repository contracts;
- account and session recovery;
- atomic account-plus-credential provisioning;
- dedicated encrypted secret-store contract and SQLite adapter;
- OIDC client, consent, subject, code, and token-family persistence;
- transactional verifier replacement and session-credential revocation on
  password change;
- transactional revocation on account disable, password reset, and uninstall.

### P4A — account authentication

- account chooser and explicit session creation;
- authenticator implementations for platform requests;
- lock, logout, reauthentication, and account switching;
- audit events tied to `UserId`, `SessionId`, and `CorrelationId`.

### P4B — OpenID Provider

- discovery and JWKS;
- install-time client registration;
- authorization and consent transactions;
- token, userinfo, revocation, and logout endpoints;
- pairwise subjects, signing-key rotation, refresh-token rotation, and replay
  response;
- interoperability tests with a generic OIDC client and Nextcloud.

### P4C — public API transports

- HTTP and IPC adapters around the transport-neutral platform API;
- secure browser session cookies for the React OS frontend;
- WebSocket authentication and session revocation propagation;
- no OAuth tokens in URL query strings or persistent browser storage.

### P5 — OS integration

- stable local issuer hostname and TLS trust;
- reverse-proxy routing usable by both the browser and app containers;
- runtime secret injection;
- Buildroot service supervision, database ownership, backup, and key handling.

## Acceptance criteria

The account and OIDC milestone is complete only when:

- two local accounts can remain separate across reboot;
- each account can independently authorize the same installed app;
- Nextcloud signs in through discovery and Authorization Code with PKCE;
- consent, logout, password reset, account disable, and app uninstall revoke the
  correct sessions and token families;
- a captured code or rotated refresh token cannot be replayed;
- an app cannot register a wildcard or foreign redirect URI;
- a client cannot use an ID token or access token issued for another client;
- signing-key rotation keeps the advertised JWKS and verification behavior
  coherent;
- account switching never changes the identity attached to existing operations;
- offline local login and local OIDC work without a cloud dependency.

## Normative references

- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0-18.html)
- [OpenID Connect Discovery 1.0](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [RFC 8414 — OAuth 2.0 Authorization Server Metadata](https://www.rfc-editor.org/rfc/rfc8414.html)
- [RFC 8252 — OAuth 2.0 for Native Apps](https://www.rfc-editor.org/rfc/rfc8252.html)
- [RFC 9700 — OAuth 2.0 Security Best Current Practice](https://www.rfc-editor.org/rfc/rfc9700.html)
- [RFC 10017 — OAuth 2.0 for Browser-Based Applications](https://www.rfc-editor.org/rfc/rfc10017.html)
