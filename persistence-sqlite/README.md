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

## SQLite linking

The default `bundled` feature compiles SQLite with the crate. This keeps host
development and CI reproducible.

For a Buildroot image that provides `libsqlite3`, build without default
features:

```text
cargo build -p rumahl-persistence-sqlite --no-default-features
```

The service that owns this repository remains responsible for choosing and
creating the parent state directory. No host path is embedded in the domain
model or in this adapter.
