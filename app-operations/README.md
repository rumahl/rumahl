# rumahl app operations

`rumahl-app-operations` coordinates app installation across persistence
boundaries without pretending that they share one transaction.

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

The current runner executes install operations. The core state machine already
models update, uninstall, and compensation, but their provider-specific policy
is not yet connected here.
