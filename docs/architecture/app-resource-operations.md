# App resource operations

Status: durable operation state machine and SQLite journal implemented;
resource execution and startup reconciliation remain to be connected.

## Purpose

Installing, updating, or uninstalling an app crosses persistence boundaries:

- the platform snapshot;
- optional app databases;
- an optional OIDC client;
- later, runtime instances and secret delivery.

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
optional OIDC client
platform snapshot
```

An app that declares neither optional feature receives only the platform
snapshot step. Database use never creates an OIDC step, and OIDC use never
creates a database step. The platform snapshot is always last, so live durable
platform state cannot advertise resources whose provider step has not
completed.

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

`SqliteAppOperationRepository` persists an operation and all ordered resource
steps in one `IMMEDIATE` SQLite transaction with WAL and `synchronous=FULL`.
It lists incomplete operations in stable start order for startup recovery.
Stored identifiers and states are parsed back through the core constructors;
invalid or unknown data fails closed.

## Remaining integration

The next runtime slice must:

1. create the journal entry before the first external side effect;
2. persist `applying` before and `applied` after each participant call;
3. use the implemented idempotent SQLite database reconciliation and add the
   equivalent secret-safe OIDC reconciliation after startup;
4. store the platform snapshot as the final resource step;
5. publish cloned in-memory `PlatformState` only after the snapshot step;
6. retain enough audit context without recording secrets or database queries.

Provider-specific credentials, filesystem paths, and connection types remain
outside this operation model.
