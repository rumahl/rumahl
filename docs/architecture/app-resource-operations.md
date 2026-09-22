# App resource operations

Status: durable operation state machine, SQLite journal, restart-safe install
and uninstall execution, resumable install compensation, authenticated
Buildroot runtime-provider channel, and a replaceable hardened Docker target
implemented. Update execution, package-to-image import, and runtime-secret
namespace materialization remain to be connected.

## Purpose

Installing, updating, or uninstalling an app crosses persistence boundaries:

- the platform snapshot;
- optional app databases;
- a prepared and activated runtime instance;
- an optional OIDC client and runtime-secret delivery.

These resources cannot share one database transaction. rumahl therefore uses
a durable operation journal. The journal does not claim that independent
providers are atomically committed. It records enough progress to determine
whether an interrupted operation must continue forward, reconcile an
ambiguous step, or compensate already applied steps.

## Plans

`AppOperation::for_app` derives the resource plan only from validated app
declarations. The current install order is:

```text
optional app databases
prepare runtime instance and secret namespace
optional OIDC client
activate runtime instance
platform snapshot
```

Every app receives the two runtime steps. An app that declares neither optional
feature is still prepared and activated before its platform snapshot is
published. Database use never creates an OIDC step, and OIDC use never creates
a database step. The platform snapshot is always last, so live durable platform
state cannot advertise resources whose provider step has not completed.

Uninstall uses its own safe order: stop the runtime, revoke and remove runtime
secrets, remove the prepared runtime, retain databases, then persist the
removed snapshot. Install compensation naturally performs the equivalent work
in reverse install order.

The same journal model supports install, update, and uninstall operations.
Execution policy remains operation-specific: an install can compensate on a
terminal provider failure, while a security-sensitive uninstall normally
continues forward until data access is retained and credentials are revoked.

## Crash-visible states

Every resource step is one of:

```text
pending
applying
applied
compensation-pending
compensating
compensated
```

The journal stores `applying` before invoking a provider and `applied` after
the provider confirms success. A crash in between is deliberately ambiguous;
startup reconciliation must inspect the provider and complete the same
idempotent action instead of guessing that it failed.

Compensation is recorded in reverse plan order. A step that was never touched
remains `pending`, while an applied or ambiguous step becomes
`compensation-pending`. Completion is allowed only after no compensation work
remains.

Operations have the phases `applying`, `committed`, `compensating`, and
`compensated`. Only committed and compensated operations are terminal.

## Concurrency and persistence

Every state transition increments an operation revision. The
`AppOperationRepository` contract requires an optimistic compare-and-swap
against the immediately preceding revision. A stale process therefore cannot
overwrite recovery progress made by another process.

`SqliteAppOperationRepository` persists an operation, its validated target app,
and all ordered resource steps in one `IMMEDIATE` SQLite transaction with WAL
and `synchronous=FULL`. Persisting the target is essential: before the final
platform snapshot exists, an `InstallationId` alone cannot reconstruct the
manifest and derived registries after a process restart. Existing journals are
migrated with a nullable target column and an expanded runtime-resource
constraint. The runner rejects incomplete legacy plans that lack the runtime
preparation/activation boundary instead of delivering secrets to a namespace
that may not exist.

The repository lists incomplete operations in stable start order for startup
recovery. Stored targets, identifiers, and states are parsed back through the
core constructors; invalid or unknown data fails closed.

## Operation runner

For install, `rumahl-app-operations::AppOperationRunner`:

1. create the journal entry before the first external side effect;
2. persist `applying` before and `applied` after each participant call;
3. invoke the implemented idempotent SQLite database reconciliation and
   prepare an isolated runtime without starting app code;
4. run secret-safe OIDC registration recovery and deliver a confidential OIDC
   secret through an idempotent privileged runtime channel;
5. activate the prepared runtime only after secret delivery succeeds;
6. store the platform snapshot as the final resource step;
7. commit the journal and only then publish cloned in-memory `PlatformState`.

If a provider or delivery attempt fails, the operation remains in its durable
`applying` state. Startup recovery reconstructs the staged installation from
the journal target and replays the ambiguous participant. Runtime delivery is
keyed by operation, installation, and client identity, so the same recovered
secret is acknowledged while a changed value must fail closed.

The SQLite integration test interrupts the first secret delivery, drops every
repository, reopens the journal, database provider, OIDC repository, snapshot
repository, and encrypted secret store, then completes the same installation
with the same client identity and secret digest.

For uninstall, the runner stages removal from platform state and grants, then:

1. deactivates the runtime so app code can no longer consume credentials;
2. removes runtime OIDC material, revokes the active client, and deletes the
   encrypted client secret;
3. removes the prepared runtime and its namespace;
4. moves app databases to retained, inaccessible storage;
5. stores the removed platform snapshot;
6. commits the journal before publishing the staged state and grants.

An interrupted uninstall always resumes forward. Its provider actions are
idempotent, including retained databases and already-removed credentials. The
SQLite integration test injects a failure during runtime-secret removal,
reopens every repository, and verifies that the same uninstall commits.

Install failures remain `applying` until the caller either retries them or
explicitly classifies them as terminal through `compensate_install`. That
transition is durable. Compensation replays the touched resources in reverse
order, stores each compensation transition, and publishes removed state only
after the operation reaches `compensated`. Startup recovery also resumes an
interrupted compensation.

## Remaining integration

- update execution with version validation, database migration/backup, health
  checks, and rollback;
- the higher-level policy that classifies an install failure as retryable or
  terminal before invoking compensation;
- package verification/import resolver and namespace-specific secret target
  behind the authenticated Buildroot supervisor channels;
- audit events without secrets, credentials, or database queries.

Provider-specific credentials, filesystem paths, and connection types remain
outside this operation model.
