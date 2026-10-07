# Storage, updates and the system database

rumahl OS separates the system from durable user and system data so that an
update can replace the operating system without touching the data, and so the
system database stays private and consistent.

## Volumes

The product image uses dedicated volumes with independent lifecycles:

| Volume | Mount | Contents | Lifecycle |
| --- | --- | --- | --- |
| System slot A / B | `/` (read-only), `/rumahl` | OS and rumahl services | replaced by an update (A/B) |
| `home` | `/home` | user data | persistent, backed up |
| `apps` | `/apps` | installed apps and assets | persistent, backed up |
| `state` | `/var/lib/rumahl` | databases and secrets | persistent, private, backed up |

`/rumahl` lives on the read-only system volume, so it cannot change while the
system runs. `tmpfs` covers `/tmp`, `/run` and volatile logs.

The `home` and `apps` volumes are the primary backup and restore targets; the
`state` volume is also backed up (accounts, secrets) but is never exposed.

## The system database

The system databases (`accounts.sqlite`, `platform.sqlite`,
`preferences.sqlite`, `files.sqlite`) and the secret store live on the `state`
volume only.

**Not reachable.** The `state` volume is never mounted, symlinked or bind-mounted
into the user- or app-visible path namespace. Only the platform service mounts it,
owner-only (`0700`). The path/grant policy denies every access and apps never
receive a handle into it. Field- and process-level encryption protect the values.

Sensitive column values (personal file content) are additionally sealed per row
with AES-256-GCM under the OS key provider, bound to the owning row. A deployment
without a TPM key stores them as plaintext and logs that encryption is disabled;
the `encrypted` column lets sealed and legacy plaintext rows coexist.

**A/B safe.** Both slots share the same `state` volume. Every database records
its schema version in `PRAGMA user_version`; on startup the platform stamps a
fresh (or pre-versioning) database and refuses to run against one written by a
newer binary (`verify_schema`, `SCHEMA_VERSION`). A newer slot migrates in a
transaction and bumps the version; an older slot then refuses to start instead of
corrupting the database.

**Cleanly synchronised.** Only one platform service may write at a time. Before a
slot switch or update the service checkpoints WAL and `fsync`s. Backups use a
consistent snapshot (the SQLite online backup API, `backup_database`), never a
live file copy. `checkpoint_and_sync` flushes and `fsync`s a database in place;
`rumahl-platform-service backup DESTINATION` schema-checks every state database
and snapshots each one into an owner-only (`0700`) directory.

## Updates

Updates write the inactive system slot; the shared data volumes are untouched.
The bootloader selects the active slot. Because the data schema is versioned and
migrated explicitly, an older slot can detect an incompatible database and refuse
to run instead of corrupting it. Signed updates, boot verification and rollback
policy are separate milestones.

### Update execution boundary

This milestone deliberately splits *data safety* (implemented) from *slot
execution* (a later milestone):

Implemented now.

- The `state`, `home` and `apps` volumes are separate from the system slot, so an
  update can replace the OS without touching them.
- Every database records `PRAGMA user_version` (`SCHEMA_VERSION`); `verify_schema`
  stamps fresh databases and refuses one written by a newer binary. A newer slot
  migrates in a transaction and bumps the version; an older slot then refuses to
  start.
- `checkpoint_and_sync` and `backup_state` produce a consistent, durable snapshot
  and are the supported way to preserve data before a switch.

Deferred to update execution.

- Writing the inactive slot, computing and verifying its signature and the boot
  artifact.
- The bootloader slot switch and rollback policy, and marking a slot good only
  after the platform and data migrations succeed.
- Refusing an update whose required `SCHEMA_VERSION` is higher than the running
  binary understands, before activating the slot.

The contract between the two is narrow: update execution must (1) never touch the
data volumes while writing a slot, (2) take a `backup_state` snapshot before the
first schema migration in a new slot, and (3) treat a `verify_schema` refusal as a
failed update to roll back, never as corruption to force through.

## Development image (approximation)

The QEMU development image has a single read-only system disk and a single
persistent data disk. It approximates dedicated volumes with bind mounts:
`/var/home` to `/home` and `/var/apps` to `/apps`, while `/var/lib/rumahl` holds
the state. The product image splits these into dedicated volumes.

## Related

- [Filesystem layout](filesystem-layout.md)
- [App databases](app-databases.md)
- [Secret store](secret-store.md)
