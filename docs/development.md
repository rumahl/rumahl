# Developer server

The developer server runs the real Rust platform, SQLite login and sessions,
React SSR and Vite behind one local HTTPS origin. React components use Fast
Refresh; CSS updates in place. Rust changes trigger an incremental build and a
platform restart after a successful build. A failed compilation leaves the last
running platform available. Production builds, image services and databases
are independent of this environment.

## Start

Use Linux or WSL2 with the repository's Rust toolchain, Node 22.12+ (or Node
24), pnpm 11.19.0 and OpenSSL. Node needs ICU/`Intl`, including on rumahl OS.
No Docker, nginx, root access or global watch utility is needed.

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
```

Open `https://localhost:8443`. On first start the server creates an isolated
`developer` account with a random password. Read it from
`.rumahl-dev/credentials.json` (owner-only permissions). Accounts and sessions
survive restarts. No default password is embedded in the sources or image.

The first Cargo build takes the usual time; subsequent builds reuse Cargo's
incremental cache. Frontend code is transformed on demand, without a production
bundle build. Ctrl+C stops the managed processes and removes their temporary
sockets. A state-directory lock prevents two servers sharing the same database.

### HTTPS trust

The server creates a 30-day localhost test certificate under `.rumahl-dev/tls`.
Trust its public `localhost.pem` in a dedicated development browser profile.
Alternatively supply your existing locally trusted localhost certificate:

```sh
pnpm --dir frontend dev --cert /absolute/path/localhost.pem --key /absolute/path/localhost-key.pem
```

The certificate must cover `localhost`. Certificate creation uses OpenSSL only
when no certificate/key pair is supplied. To rotate the generated certificate,
stop the developer server, remove only `.rumahl-dev/tls`, and start again.
Do not disable TLS verification or change the Secure/HttpOnly cookie posture.
A trusted certificate is also necessary for reliable WSS hot reload.

### Daily workflow

- Edit shell components or CSS: Vite delivers a hot update. React preserves
  component state where its Fast Refresh boundaries allow it.
- Edit shared contracts or non-component client modules: Vite may reload the
  page. Rust remains authoritative for snapshots and authorization.
- Edit SSR source: the next request loads the changed module and the browser
  receives a reload notification.
- Edit Rust files or Cargo manifests: the watcher debounces changes, rebuilds
  `rumahl-platform-service` and replaces the process only after compilation
  succeeds. The browser reloads when the new socket is ready. Requests during
  the short restart can return 503; sessions remain in SQLite.
- Edit the developer launcher or Vite configuration: restart the developer
  command so all process/configuration changes take effect together.

Use `--poll` for shared folders or unreliable filesystem notifications:

```sh
pnpm --dir frontend dev --poll --port 9443
```

Use `--no-watch` to disable automatic Rust builds while retaining frontend HMR.
`--port 0` selects an available port and prints the resulting origin. The
selected origin, certificate path and development build ID are recorded in
`.rumahl-dev/server.json` for tooling. A stable port is more convenient for a
browser bookmark.

The same per-run development build ID is supplied to Rust, SSR and the client.
It stays stable during hot updates and changes on a full developer-server
restart. Production continues to use content-derived build IDs and immutable
bundles. Do not use development IDs or source-module URLs for a release.

## rumahl OS / prebuilt service

The platform executable accepts exactly the production configuration and Unix
socket protocol. To develop against a target-compatible prebuilt binary:

```sh
pnpm --dir frontend dev \
  --binary /usr/bin/rumahl-platform-service \
  --state-dir /var/lib/rumahl-development \
  --cert /path/to/localhost.pem --key /path/to/localhost-key.pem
```

This starts a separate platform instance and does not stop or reconfigure the
installed OS services. Run as a normal development user who owns the state
folder (mode 0700). It never uses `/var/lib/rumahl` for data. The checkout and
frontend dependencies must be writable; frontend packages must be installed
for the target architecture. Node, the checkout and Vite dependencies are still
required, but Cargo and the Rust compiler are not required in `--binary` mode.
Restart the command after replacing that binary. A binary/schema mismatch must
be resolved by using matching sources and binary, not by bypassing validation.

The stock image intentionally does not install a compiler, pnpm, Vite or this
developer launcher. Provision these into a separate development environment
when needed. For working directly on a read-only OS root, use a writable
checkout on the data volume. Native full-stack development is Linux/WSL2;
macOS/Windows users can access a Linux development host through SSH:

```sh
ssh -L 8443:127.0.0.1:8443 developer@development-host
```

Run the server on that host and open `https://localhost:8443` locally, trusting
that host's public development certificate. The listener deliberately binds
only `127.0.0.1`: a dev server serves source code and must not be exposed as the
public OS interface. No relaxed remote-host/CORS wildcard is enabled.

## Isolation and production compatibility

`.rumahl-dev/` is Git-ignored. A custom `--state-dir` must be an empty directory
or an existing rumahl development directory. Runtime sockets live in a fresh
private temporary directory. Credentials are never passed to Vite middleware
or the renderer. The edge preserves browser Host/Origin for Rust's existing
checks, gates WebSocket origins and rejects foreign hosts.

The renderer uses the same request bounds, snapshot validation, React render
function and response handling as production. Development adds Vite's client
and React preamble; its CSP allows nonce-bearing HMR styles. Production CSP and CSRF checks remain strict. Both paths use `same-origin`
referrer policy so native login/logout forms retain their Origin header. Module access is
limited to the frontend workspace; private state and TLS files are denied.
There is no unauthenticated development API that performs privileged actions.

The existing fixture-only demo remains available with
`pnpm --dir frontend dev:demo`. It is not a substitute for the authenticated
full-stack developer server.

## Checks

```sh
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend test
pnpm --dir frontend build
pnpm --dir frontend --filter @rumahl/ssr test:smoke
cargo build --locked -p rumahl-platform-service
python3 tests/e2e/development/check.py
```

The developer integration check starts the real binary in prebuilt mode, uses
verified HTTPS, checks login/SSR, origin and file boundaries, a live HMR
WebSocket, changed SSR output, single-runner locking and persisted sessions. It
edits only a disposable frontend copy and does not change your checkout.
`RUMAHL_SERVICE_BIN` and `NODE_BIN` can select local test executables.

For the Chromium checks used by CI (browser download required only for tests):

```sh
pnpm --dir frontend exec playwright install chromium
python3 tests/e2e/development/check.py --browser
```

This also exercises native form login, hydration, React state preservation and
CSS hot updates. Chromium trusts only the generated test certificate's public
key. Linux may require Playwright's documented browser system dependencies.

The release/image checks remain separate: use the production HTTPS test and
Buildroot acceptance described in their READMEs before shipping an image.
