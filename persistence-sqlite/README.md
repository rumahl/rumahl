# rumahl SQLite persistence

`rumahl-persistence-sqlite` implements the `PlatformSnapshotRepository`
contract from `rumahl-core`.

The repository stores one versioned platform snapshot in a SQLite transaction.
The snapshot contains installed apps and permission grants. Derived registries
are intentionally not persisted; `PlatformRecovery` rebuilds them from the
validated installed-app state after startup.

The wire schema is private to this adapter and rejects unknown fields,
malformed domain identifiers, mismatched schema versions, stale app grants,
and invalid manifests before live state is changed.

The crate also implements `AccountStateRepository` using dedicated relational
tables for local accounts and their sessions. Account and session metadata is
replaced in one immediate transaction, so an account lock and its session
revocations survive together. Credentials, tokens, client secrets, and signing
keys are intentionally outside both repositories.

Opaque OS session credentials use a third repository. Only a SHA-256 digest of
each randomly generated 256-bit token is stored; the clear token is returned
once to the HTTP, IPC, or native transport. Revoking a session invalidates all
of its stored transport credentials, while the platform API independently
revalidates the account and session on every request.

Password authentication is an optional local login adapter, not part of the
account identity itself. Its repository stores Argon2id PHC verifiers and the
persistent retry state per `UserId`. Attempt reservation, lockout updates, and
success resets are transactional; plaintext passwords never enter SQLite.

`SqliteLocalAccountAdministrationRepository` is the cross-table transaction
boundary for local identity administration. Initial account and password
creation commits together. A password change replaces the verifier, resets its
retry state, revokes all live account-session rows, and invalidates every
opaque transport credential for those sessions before committing. The caller
swaps its staged in-memory `AccountState` only after this transaction succeeds.

OIDC client registrations use dedicated relational tables keyed by generated
`client_id` and `InstallationId`. Redirect URIs, explicit client type, allowed
scopes, creation/revocation state, and only the digest of a confidential client
secret are persisted. A unique installation constraint prevents parallel
client identities for the same installed app. Revoked registrations are not
returned by active-client lookups.

## SQLite linking

The default `bundled` feature compiles SQLite with the crate. This keeps host
development and CI reproducible.

For a Buildroot image that provides `libsqlite3`, build without default
features:

```text
cargo build -p rumahl-persistence-sqlite --no-default-features
```

## App database provider

`SqliteAppDatabaseProvider` is the first physical implementation of the
engine-neutral `AppDatabaseProvider` contract. It provisions all logical
databases for one installation in a private staging directory and exposes them
atomically by renaming that directory into the active namespace. Different
`InstallationId` values never share files, even when their logical database IDs
are identical.

Provisioning is safe to replay after an interrupted operation. A matching
active installation is preserved, incomplete staging is rebuilt, and retained,
conflicting, or declaration-mismatched state fails closed. Retain and restore
are likewise idempotent for their already-reached target state.

The adapter returns a configured `rusqlite::Connection` only for an active
`AppDatabaseBinding`. Retaining an installation moves its complete database
directory out of the active namespace; restoring is permitted only for the
same installation identity. The provider root is supplied by the OS runtime
and is never derived from manifest data.

This adapter is suitable for local embedded app data and the future
authenticated React platform-data service. Container connection delivery and
managed PostgreSQL remain separate runtime adapters; neither changes the core
manifest contract.

## App operation journal

`SqliteAppOperationRepository` durably stores ordered app-resource steps for
install, update, and uninstall operations. Each transition uses an optimistic
revision check and updates the operation plus all steps in one SQLite
transaction. Incomplete operations remain queryable after restart, including
steps interrupted while applying or compensating.

The journal is a recovery mechanism across independent providers, not a false
cross-filesystem transaction. Provider execution and startup reconciliation
are layered above this repository.

## Encrypted secret store

`SqliteSecretStore` encrypts each installation-bound secret with AES-256-GCM.
The secret ID, complete app identity, purpose, key ID, and creation timestamp
are authenticated as associated data. SQLite contains only ciphertext, a
random nonce, metadata, and the non-secret key ID.

Encryption keys come from `SecretEncryptionKeyProvider`; they are never stored
in SQLite. The adapter supports an active key for writes and historical lookup
by key ID for reads and rotation. The production Buildroot key provider is a
separate integration responsibility.

The service that owns this repository remains responsible for choosing and
creating the parent state directory. No host path is embedded in the domain
model or in this adapter.
