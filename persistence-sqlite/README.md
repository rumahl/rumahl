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
