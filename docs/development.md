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
eee```

Open `https://localhost:8443`. On first start the server creates an isolated
`developer` account with a random password. Read it from
`.rumahl-dev/credentials.json` (owner-only permissions). Accounts and sessions
survive restarts. No default password is embedded in the sources or image.

The first Cargo build takes the usual time; subsequent builds reuse Cargo's
incremental cache. Frontend code is transformed on demand, without a production
bundle build. Ctrl+C stops the managed processes and removes their temporary
sockets. A state-directory lock prevents two servers sharing the same database.

### HTTPS trust

The server creates a 365-day localhost test certificate under `.rumahl-dev/tls`.
It renews the generated certificate automatically when it expires or enters its
last seven days; the renewal keeps the existing private key, but the changed
public certificate still has to be trusted in the development browser profile.
Alternatively supply your existing locally trusted localhost certificate:

```sh
pnpm --dir frontend dev --cert /absolute/path/localhost.pem --key /absolute/path/localhost-key.pem
```

The certificate must cover `localhost`. Certificate creation uses OpenSSL only
when no certificate/key pair is supplied. To rotate the generated certificate,
stop the developer server, remove only `.rumahl-dev/tls`, and start again.
Do not disable TLS verification or change the Secure/HttpOnly cookie posture.
A trusted certificate is also necessary for reliable WSS hot reload and Service
Worker registration. Merely continuing past the browser certificate warning may
load the login page while Chromium rejects the worker script with SecurityError.
Use a locally trusted certificate via `--cert` and `--key` for offline development.
Settings → This device shows registration errors, worker version and cache status.

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

The fixture-only demo remains available with
`pnpm --dir frontend dev:demo`. It includes sample apps, files and widgets and
persists state in the browser. It is not a substitute for the authenticated
full-stack developer server. Static builds for the public demo are produced
with `pnpm --dir frontend/packages/shell build:demo` and
`build:demo-widget`; see `frontend/packages/shell/src/demo/README.md`.

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

## Shell routes and modes

Use `/app/files` for a direct first-party app route and
`/settings/display` for presentation preferences. The top bar changes the mode
for this browser profile; Settings can save it for the whole account or this
device. The URL stays unchanged. Browser reload and Back/Forward preserve the route; login
returns to a validated deep link. See [shell routing](architecture/shell-routing.md)
for the module layout, app registration boundary and remaining integration work.

## Installed web apps

Desktop and launcher share the authorized app catalog and `/app/:appId/*` host.
A backend installation, explicit launch grant and published web assets are
required; the shell does not import an app's React code. See
[app hosting](architecture/app-hosting.md) for the contract and current limits.

The developer certificate now covers both `localhost` and `*.apps.localhost`.
On the first start with an older or expired generated certificate, the runner
regenerates it. Trust the displayed certificate again; custom `--cert` certificates must
cover both names. The browser must resolve installation subdomains beneath
`apps.localhost` to loopback. Shell and apps use the same developer HTTPS port.

Install a real, signed app into the running developer state with:

```sh
pnpm --dir frontend dev:app
```

It builds and signs the built-in development app, installs it into
`.rumahl-dev/data` and grants launch access to the existing accounts, so it
appears in the launcher and Settings → Apps. This runs the same signed-package path as production,
unlike the integration fixture below (which is not an installer).

The integration fixture is only for disposable developer state; it refuses to
seed a platform that already has installations. It is not an installer:

```sh
cargo build -p rumahl-platform-service --bins --example app_host_fixture
# While the developer server is running (adjust the state directory as needed):
target/debug/examples/app_host_fixture seed "$HOME/.local/state/rumahl-dev/data" developer
# Open the launcher; "Isolated test app" is granted only to the developer account.
target/debug/examples/app_host_fixture revoke "$HOME/.local/state/rumahl-dev/data"
```

Both `tests/e2e/shell/check.py` and
`tests/e2e/development/check.py --browser` require this fixture executable.
They use temporary state and check real TLS, app hosting and grant revocation.

## Synchronized user and device preferences

In **Settings → Shell mode**, choose **My account · all devices** or
**This browser profile**. A device override takes priority; **Use account setting**
removes it. User preferences automatically follow the same account on other
browser profiles connected to this rumahl OS instance. Multiple tabs of the same
profile share its device preferences. Other users keep independent settings.

Modes and preference caches use Local Storage, with Session Storage fallback.
Rust persists authoritative values in `data/preferences.sqlite` beneath the
developer state directory (production: `$RUMAHL_STATE_DIR/preferences.sqlite`).
Active pages refresh every two seconds, hidden pages every ten seconds, and
focus/local storage notifications trigger an immediate refresh. Concurrent stale
writes return a visible conflict instead of overwriting a newer change.

A “device” currently means a browser profile on this origin, not hardware
fingerprinting. Clearing storage creates a new profile; browsers on the same
physical device remain separate. This is local OS synchronization, not cloud
synchronization between separate rumahl servers. See
[shell preferences](architecture/shell-preferences.md) for the versioned API and
extension rules. `shell.mode` is the first supported preference.

### RRE — rumahl Recovery Environment (`/recovery`)

The recovery surface is served directly by the Rust gateway. It needs neither the
React desktop nor the SSR renderer. The HTML and controller live in
`platform-web/src/recovery.html` and `platform-web/src/recovery.js` and are embedded
at Rust build time, so a backend rebuild is required after changes.

Opening the environment runs read-only checks for components, app services,
resources, storage, network, logs, recent app crashes and boot errors. Checks that
cannot run are reported as incomplete. A JSON report exports readings, findings and
the last repair result; it contains no session tokens and no activity log.

Repair only starts authorized, stopped `always-on` apps. Running services and normal
`on-demand` apps are not restarted. Results are shown per service and changes are
recorded in the audit log. Under **Apps & services** individual apps can be started,
stopped and restarted, and apps that keep crashing can be quarantined; a quarantined
app is never restarted automatically. Platform and renderer processes have no
separate restart action. Device restarts and power-off go through the host broker
described below. SMART starts a host test and is not a completed integrity check.
The storage page offers a read-only filesystem check (`fsck`) of unmounted devices;
mounted filesystems are reported as needing a maintenance reboot. The system page
shows build and version information and, through the host broker, can roll back to
the newest compatible snapshot, erase the device (factory reset / reinstall) or
power it off.

Safe mode blocks launch, start and recovery-restart requests as well as automatic
app repair through the gateway. Already running processes are not stopped
automatically. The setting does not replace host boot isolation.

Manual verification after a build: open `/recovery` signed in and signed out, run
diagnostics with the runtime supervisor unavailable, stop/start an authorized app,
enable safe mode and attempt another start. Also check with a failed SSR and a
narrow browser window. Rust regression checks:
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`.

RRE is a standalone mini desktop: a plain background, a system bar and a dock at the
bottom. System status, diagnostics, apps & services, storage & disks, network, logs,
app crashes, boot & errors, system & update, boot options, activity and backups open
in separate windows. Several tools can stay visible at once. Windows can be moved by
their title bar (also with the arrow keys when the title bar is focused), maximised,
minimised and closed. The dock brings minimised windows back; dots mark open tools.
On narrow screens the active window fills the workspace.

Diagnostics run read-only in the background without opening a window. Window controls
and the dock stay usable during API requests. Closing or minimising a tool window
neither stops apps nor cancels an already submitted repair. "Refresh" reloads the
tool data. The surface needs no shell assets; individual desktop themes are not
applied.

### RRE: host restart, power-off and full state backup

"Activity" is its own app with live events, an outcome filter and paging. "System
status" shows check states, "Diagnostics" shows findings and repairs, "Apps &
services" controls runtimes. "Boot options" offers a normal and a safe **device
restart** plus a **power off**, "Backups" creates and restores local state backups
and can schedule automatic backups. Windows still open only from the dock.

Host actions require the new `rumahl-recovery.service`. The Rust gateway talks only
to its Unix socket. The broker checks the platform service's peer UID and the signed
in user ID against the root-owned file `/etc/rumahl/recovery.json`. The shipped
`operators` list is **empty**: at provisioning time add explicitly authorised
accounts as UUIDs. The file must be owned by root and must not be writable by others.
A session or an extended UI mode alone grants no maintenance rights. Adding it to the
image requires a new image build, not only a rebuild of the Rust gateway. The normal
frontend dev server has no host broker, so RRE shows those functions as unavailable.

The broker accepts only `status`, `reboot`, `poweroff`, `backup`, `restore`, `fsck`,
`reset`, `reinstall`, `rollback` and `schedule`. Paths and shell commands cannot be
passed from the browser. The interface first confirms the **acceptance** of an
operation. While services are stopped the connection is interrupted; reopen RRE
afterwards and check the stored operation status. An HTTP 200 check is not treated as
a successful boot.

A restart or power-off is signalled by the server, never guessed by the client. The
broker writes a lifecycle intent before stopping writers; on a clean stop the
platform service broadcasts a `system_lifecycle` WebSocket event to every connected
client, which then shows "restarting" or "shut down". Without that signal the
offline screen only distinguishes "this device is offline" from "rumahl OS is
unreachable" (via a service worker connectivity probe) and polls patiently. A plain
`systemctl stop/restart` defaults to "restarting"; a crash sends no signal and is
reported as unreachable.

A full state backup covers `/var/lib/rumahl`, `/home`, `/apps`, `/var/lib/docker`
and `/var/lib/rumahl-runtime-images`. All five directories must be present. Before
the backup the rumahl services and all containers of this appliance host are stopped.
The backup preserves owners, ACLs, extended attributes and Docker whiteouts.
Additional mounts inside the volumes are rejected instead of silently producing
incomplete backups. GNU tar with ACL/xattr support and Python with SQLite are added
to the image (see
[Buildroot tar configuration](https://github.com/buildroot/buildroot/blob/master/package/tar/tar.mk)).

Backups are stored root-private in `/var/lib/rumahl-recovery/backups`. They are
local restore points, not protection against loss of the disk. The OS image, TLS
provisioning and host configuration are not replaced. Restoration is bound to the
same device, platform build, platform configuration and, where present, the same TPM
key object. Moving a device or importing foreign archives is not offered.

Restore first checks the manifest, scope, SHA-256 and archive paths. After writers
are stopped an additional backup of the current state is created. All volumes are
extracted onto their target filesystem before replacement; state databases are
checked for integrity. Session tokens and OIDC login transactions are discarded.
After a successful restore the device reboots into safe mode, where Docker and the
app supervisor stay stopped by systemd conditions until a normal device restart is
requested.

**Interrupted restore:** `/var/lib/rumahl-recovery/restore-pending.json` locks all
affected services even after a power failure. The gateway also stays stopped; this
needs local maintenance access. The previous state is noted as `safetyBackupId` in
`/var/lib/rumahl-recovery/job.json`. After review, with the broker stopped, root can
re-apply a verified restore point with
`python3 /usr/libexec/rumahl-recovery.py --restore-offline BACKUP_ID`. The command
leaves services stopped; reboot afterwards. Do not simply delete the lock marker. A
restore needs room for the current state, the extra backup and the extracted data;
lack of space leads to rejection or the locked maintenance state.

Offline tests that leave the host untouched:
`python3 -m unittest discover -s platform-buildroot/recovery -p 'test_*.py' -v`.
Device restart, mount behaviour and the systemd flow must additionally be verified in
a disposable QEMU instance before activating the broker on a device with important
data.
