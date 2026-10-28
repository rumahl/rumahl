# User and device presentation preferences

Presentation settings are separate from permissions, identity and app package
state. The first supported setting is `shell.mode` (`desktop` or `launcher`).
User values synchronize across sessions and browser profiles of the same account
on one rumahl OS instance. Device overrides affect only that account in one
browser profile. Effective value: device override → user value → stock default
(`desktop`). Removing the override resumes inheritance immediately.

## Persistence and identity

Rust owns the typed contract (`core/src/shell_preferences.rs`) and persistence
adapter (`SqliteShellPreferences`). `$RUMAHL_STATE_DIR/preferences.sqlite` stores
user rows and device override rows. The authenticated session determines the
user ID; API callers cannot select another user. A random UUID v4 stored as
`rumahl.browser-profile.v1` identifies the browser's settings namespace. It is
not a trusted hardware identity, credential, permission or device registration.
Different users using the same profile remain isolated by the server's user key.

The shell also stores confirmed preferences under
`rumahl.preferences.v1:<user-id>:<profile-id>` in Local Storage (Session Storage
fallback). It never applies a previous user's cache before the API identifies
the authenticated account, and it never places session credentials in storage.
Blocked storage degrades to session/in-memory profile identity and is disclosed
in Settings. Clearing storage, changing origin or another browser creates a new
device profile. Actual hardware/kiosk defaults or synchronization between separate
rumahl OS servers require their own identity, authorization and transport design.

## HTTP contract v1

Both operations require the normal authenticated shell session:

- `GET /api/v1/shell/preferences?device=<uuid-v4>`
- `PUT /api/v1/shell/preferences?device=<uuid-v4>`

Writes additionally require the exact shell Origin and bounded JSON:

```json
{"settingsVersion":1,"revision":0,"scope":"user","key":"shell.mode","value":"launcher"}
```

`scope` is `user` or `device`. Null removes the selected override (at user scope,
restores the default). Responses contain `settingsVersion`, authenticated
`ownerId`, a safe integer `revision`, `user.shellMode`, nullable
`device.shellMode`, and resolved `effective.shellMode`. All responses are no-store.
Unknown keys/scopes/versions and invalid values are rejected. These endpoints
are not exposed on app hosts.

A SQLite immediate transaction compares the revision before writing and advances
it atomically. Revisions are per user, including that user's device changes.
Concurrent stale writes return 409; the client loads the latest state and asks
the user to retry, never silently retries an obsolete write. Unavailable storage
returns 503; writes are not claimed as saved on failure. A maximum of 128 device
overrides per user prevents unbounded profile creation (429); removing an override
frees a slot. GET does not create rows.

## Client synchronization and routing

`preferences/ShellPreferences.tsx` is the shared provider; route views consume it
without owning transport. It refreshes every 2 seconds while visible, every 10
seconds while hidden, and on focus/visibility and same-origin storage events.
Storage events carry only an invalidation hint; the API remains authoritative.
Reads during writes cannot apply a stale result; polling and in-flight requests
are canceled on unmount. No offline write queue silently replaces remote values.

SSR and initial hydration use the same deterministic default. Preferences load
after hydration, then update presentation without navigating. The URL and app
query parameters do not encode shell mode. Legacy `mode` parameters are removed
with history replacement, never used to overwrite saved preferences. Back/Forward
navigate content independently of mode. Demo mode stores presentation locally and
makes no claim of server synchronization.

Settings exposes user/device scope, the effective mode, save failures and conflict
messages. The top-bar selector changes the current profile's device override.
Adding a new setting requires a typed core value, validation, storage migration,
versioned response/client projection, scope/reset semantics and tests. Do not turn
this into an arbitrary JSON store for credentials or authorization flags.

## Verification

SQLite tests cover inheritance, user isolation, restart, simultaneous writers and
profile limits. HTTP tests cover session authentication, CSRF, impersonation fields,
conflicts and logout. Client tests cover validation, cache isolation, polling,
cancellation and conflict refresh. Chromium exercises two independent profiles of
one account plus multiple tabs, device precedence, reload and URL-free navigation.
