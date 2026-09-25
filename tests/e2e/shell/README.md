# Local HTTPS shell acceptance

Build with `cargo build --locked -p rumahl-platform-service` and
`cd frontend && pnpm build`, then run `python3 tests/e2e/shell/check.py` from the
repository root. Requires Node, nginx, OpenSSL and Python. Optional executable
overrides: `RUMAHL_SERVICE_BIN`, `NGINX_BIN`, `NODE_BIN`.

The test starts real platform and bundled-renderer processes plus nginx on an
unused loopback HTTPS port. It creates two random test passwords, ephemeral
SQLite state and a test-only certificate verified by the HTTPS client. It never
logs the credentials, changes system trust or installs system services.

It verifies authenticated SSR, theme/client asset delivery, user separation,
process restart persistence, durable logout, revocation over a real WebSocket,
and recovery/login availability when SSR fails. Expect about 40 seconds because
WebSocket sessions have a 30-second revalidation interval. All child processes
and temporary state are cleaned up. A passing result does not substitute for a
Buildroot VM boot/reboot test.
