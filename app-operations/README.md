# rumahl app operations

`rumahl-app-operations` coordinates app installation and uninstallation across
persistence boundaries without pretending that they share one transaction.

`AppOperationRunner` stages the validated core app lifecycle, creates a durable
journal entry, then applies optional app databases, optional OIDC registration
and confidential-secret delivery, and the platform snapshot in order. It
persists `applying` before each provider call and `applied` afterwards. Live
`PlatformState` is replaced only after the snapshot and committed journal
transition succeed.

The journal includes the complete validated installation target. On startup,
the runner can therefore reconstruct an installation interrupted before its
platform snapshot existed. Ambiguous provider calls are replayed through their
idempotent contracts. Confidential OIDC delivery receives the operation ID,
installation identity, client ID, and zeroizing secret wrapper; a runtime
adapter must acknowledge an identical replay and reject changed material.

Uninstall moves app databases into retained, inaccessible storage, removes
runtime secret material, revokes the OIDC client, deletes the encrypted secret,
and stores the removed platform snapshot before publishing live state. An
interrupted uninstall continues forward during startup recovery.

Callers can explicitly classify a failed install as terminal and start durable
reverse-order compensation with `compensate_install`. Interrupted compensation
is resumed on startup. Update execution is intentionally not connected yet: it
still needs version-transition validation, database backup/migration, health
checks, and rollback policy.
