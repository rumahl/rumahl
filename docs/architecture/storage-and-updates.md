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

**A/B safe.** Both slots share the same `state` volume. Migrations are guarded by
a data-schema version decoupled from the binary version: a new slot backs up the
database before migrating, migrates in a transaction, and refuses to start on an
incompatible schema. A slot rollback restores the pre-migration backup.

**Cleanly synchronised.** Only one platform service may write at a time. Before a
slot switch or update the service checkpoints WAL and fsyncs. Backups use a
consistent snapshot (SQLite backup API or a copy-on-write volume snapshot), never
a live file copy.

## Updates

Updates write the inactive system slot; the shared data volumes are untouched.
The bootloader selects the active slot. Because the data schema is versioned and
migrated explicitly, an older slot can detect an incompatible database and refuse
to run instead of corrupting it. Signed updates, boot verification and rollback
policy are separate milestones.

## Development image (approximation)

The QEMU development image has a single read-only system disk and a single
persistent data disk. It approximates dedicated volumes with bind mounts:
`/var/home` to `/home` and `/var/apps` to `/apps`, while `/var/lib/rumahl` holds
the state. The product image splits these into dedicated volumes.

## Related

- [Filesystem layout](filesystem-layout.md)
- [App databases](app-databases.md)
- [Secret store](secret-store.md)
