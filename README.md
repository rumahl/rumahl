# rumahl OS

A local-first operating system and application platform for the home.

This repository contains the second-generation rumahl OS implementation, building on several years of experimentation across multiple earlier prototypes. The public rumahl-prototype repository represents the most recent of those prototypes and the first generation built around a dedicated operating system architecture.

Earlier iterations explored the same core ideas using a container-based architecture, with rumahl itself running as a container while orchestrating additional workloads on the host.

## Local browser milestone

The [platform service](platform-service/README.md) connects real SQLite accounts
to the React shell through HTTPS. See the [Buildroot/QEMU instructions](platform-buildroot/image/README.md)
for image integration and the [HTTPS acceptance test](tests/e2e/shell/README.md)
for a reproducible local check.

## Development

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
```

Starts the real platform and HTTPS shell with React/CSS hot reload and automatic
Rust rebuilds. See [the developer guide](docs/development.md) for first login,
local certificate trust, WSL/shared folders and running a prebuilt service on
rumahl OS. Development data stays separate from the installed system.
