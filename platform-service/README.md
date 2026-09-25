# Local browser milestone

`rumahl-platform-service` composes the existing Rust authentication, SQLite,
platform API and browser gateway. It runs on a private Unix socket and uses the
separately supervised React renderer. No fixture accounts or snapshots are
loaded by this service.

The first slice provides:

- local password login, a fresh 12-hour `__Host-` session cookie and durable logout;
- atomic session-plus-credential insertion, including a verifier/status check
  against concurrent password changes, and atomic session-plus-credential revocation;
- current account display name and installed-app count from the real repositories;
- a deterministic stock theme, matching client build ID and private/no-store HTML;
- persisted-state polling and WebSocket hints; open sessions revalidate at most
  every 30 seconds, and reconnecting clients reload authoritative data;
- independent, asset-free login and recovery pages when the renderer fails.

The service does not yet compose the optional OIDC/stream providers, execute
app-install actions or register widgets. System protection is reported as
`attention` until there is an actual health assessment. Locale is device
configuration for this slice. Recovery remains informational, with no privileged
reset action. These omissions are explicit; no fake data fills them in.

## Start

Build both sides from the same checkout:

```sh
cargo build --locked -p rumahl-platform-service
cd frontend
pnpm install --frozen-lockfile
pnpm build
```

Configuration is listed in
[`platform.env.example`](../platform-buildroot/deployment/platform.env.example).
The state directory must exist with mode `0700`. Each socket directory must be
private, with mode `0700` for a single-user development process, or `0750` and the
explicit shared group for separate service users. Existing socket files are not
silently removed; systemd owns volatile directory cleanup.

Start the renderer with `node frontend/packages/ssr/server.js` and the platform
with `rumahl-platform-service serve`. Put the supplied nginx HTTPS edge in front
of the platform socket. There is no public plaintext HTTP fallback. Public Host
and POST Origin must match `RUMAHL_PUBLIC_ORIGIN`, including a non-default port.

## Provision a local account

Provisioning is a local administrative CLI, not an unauthenticated web endpoint.
Run as the owner of the state directory, with `RUMAHL_STATE_DIR` and
`RUMAHL_PASSWORD_BLOCKLIST` set. The latter names a local UTF-8 file of prohibited,
NFC-normalized passwords, one per line. An empty blocklist is rejected. Passwords
must satisfy the existing platform policy (at least 15 characters by default).

Use a hidden Bash prompt; do not put the password in an argument or environment:

```bash
read -r -s -p 'New local password: ' rumahl_password
printf '\n'
printf '%s' "$rumahl_password" | rumahl-platform-service provision-account alice Alice --password-stdin
unset rumahl_password
```

Do not enable shell tracing around this command. There are no default accounts
or passwords. Provisioning preserves unrelated accounts and sessions rather
than rewriting the whole account snapshot.

## Verification

```sh
cargo test --locked -p rumahl-platform-service -p rumahl-persistence-sqlite
python3 tests/e2e/shell/check.py
```

The second command requires the debug platform binary, built frontend, Node,
nginx, OpenSSL and Python. It runs actual HTTPS, SQLite, gateway and SSR processes
with ephemeral local state and a test-only certificate trusted by the client.
It checks two users, immutable assets, session survival across process restarts,
WebSocket revocation and renderer-independent recovery. This is a process-level
acceptance test, not a claim that a Buildroot image has booted.

See [image build and VM instructions](../platform-buildroot/image/README.md).
