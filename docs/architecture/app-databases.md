# App databases

Status: engine-neutral core contract and first local SQLite provider.

## Purpose

Databases are an optional app resource. An app that does not declare a
database receives no database binding. Database use neither enables nor
requires OIDC; an app may use neither feature, either feature, or both.

The manifest declares logical database names only:

```yaml
databases:
  - id: primary
  - id: search-index
```

Every active database is addressed by the pair:

```text
(InstallationId, AppDatabaseId)
```

The app cannot choose a host path, socket, hostname, port, username, password,
DSN, SQLite filename, PostgreSQL database, or other storage implementation.
Those are rumahl OS policy decisions. This preserves the architecture rule
that persistence adapters remain outside domain types.

An app can declare at most eight databases. IDs are lowercase logical names;
path separators and filename suffixes are rejected. An ID never selects a
database engine, even if an app chooses a name that resembles one.

## Runtime delivery

The same declaration is delivered differently according to the trusted
runtime adapter:

- A static React web app accesses its database through an authenticated rumahl
  platform data API. The OS derives the installed app and active account from
  `OperationContext`; neither identity is accepted from browser request data.
- A container app receives a connection through the runtime secret channel.
  Connection credentials never enter the manifest, platform snapshot, React
  bundle, command line, or normal app storage.
- Native delivery remains disabled until the native runtime trust policy is
  defined.

The initial Buildroot provider may select per-installation SQLite for small
local apps and managed PostgreSQL for server workloads. That selection belongs
to the provider adapter and can change without changing `AppManifest` or app
identity.

## Lifecycle and data ownership

The core currently turns every declaration into an `AppDatabaseBinding` owned
by the concrete `AppIdentity`. `AppLifecycle` registers these bindings together
with capabilities, contributions, events, and the installed app. Any conflict
rolls back the staged platform state. Recovery rebuilds the database registry
from the persisted, validated manifest.

Uninstall removes the active binding so the app can no longer resolve the
database. Physical data must not be erased immediately. The provider will move
it into a retained, inaccessible state so reinstall recovery, backup, explicit
export, and administrator-approved purge remain possible. A new installation
receives a new `InstallationId` and must not silently inherit an old database.

Updates require a provider snapshot before schema migration. The runtime marks
the migration successful only after the updated app passes its health check;
otherwise database and runtime roll back together.

## Multi-account behavior

The physical database belongs to the app installation, not directly to an OS
account. Account separation is enforced at the app or platform-data-API layer
using the authenticated `UserId`. A container that offers its own multiuser UI
can use the optional rumahl OIDC provider to identify accounts, but OIDC is not
a prerequisite for database provisioning.

If platform-managed per-account databases are added later, they require an
explicit manifest isolation mode and separate migration/backup semantics. They
must not be inferred from whether OIDC is enabled.

## Delivery phases

### D1 — logical contract (implemented)

- `AppDatabaseId` and `AppDatabaseDeclaration`;
- optional manifest declarations with duplicate and count limits;
- per-installation `AppDatabaseRegistry` integrated into atomic app lifecycle;
- snapshot round-trip and recovery of database declarations.

### D2 — provider lifecycle (partially implemented)

Implemented:

- engine-neutral provision, access, retain, and restore provider contract;
- SQLite adapter with atomic staging-directory activation and per-installation
  file isolation;
- idempotent SQLite provisioning that preserves a matching active installation,
  rebuilds incomplete staging, and rejects conflicting physical states;
- retained databases are inaccessible until restored for the exact original
  `InstallationId`;
- durable cross-resource operation state machine and SQLite journal, including
  crash-visible apply/compensation steps and optimistic revisions;
- restart-safe installation runner connecting database provisioning, OIDC
  recovery, runtime-secret delivery, and the final platform snapshot.
- restart-safe uninstall execution that retains database data before removing
  credentials and platform state;
- resumable reverse-order install compensation after explicit terminal-failure
  classification.

Remaining:

- explicit purge policy and audit workflow;
- update execution with migration, health-check, backup, and rollback policy;
- quota and storage-health reporting;
- audit events without credentials or query contents.

### D3 — runtime access

- authenticated React/platform API adapter;
- namespace-specific target for the implemented authenticated Unix-socket
  secret channel, plus credential rotation;
- no network exposure outside the app runtime namespace;
- rate, connection, and resource limits per installation.

### D4 — Buildroot persistence

- supervised database services and durable storage ownership;
- transactional backup/restore and update rollback;
- provisioned TPM sealing objects, PCR/update policy, and recovery workflow for
  the implemented root-key adapter;
- power-loss, low-disk, corruption, and recovery tests.

## Acceptance criteria

- Apps without database declarations install without database side effects.
- Two installations can use the same logical database ID without sharing data.
- One installation cannot register the same database ID twice.
- Database bindings survive platform snapshot recovery.
- Uninstall makes every database inaccessible before the app disappears from
  live state, while physical deletion requires a separate explicit purge.
- No manifest, snapshot, log, or browser bundle contains database credentials.
