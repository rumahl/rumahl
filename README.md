# rumahl OS

A local-first operating system and application platform for the home.

rumahl OS provides a platform for running self-hosted applications while
keeping accounts, application permissions, storage, and system operations under
the control of the local system.

The platform is designed around a few core ideas:

- Local-first by default
- Applications use capabilities instead of direct host access
- First-party and third-party apps use the same platform contracts
- Multiple local user accounts
- Built-in OAuth 2.0 / OpenID Connect identity provider
- Privileged system operations remain isolated from applications

> rumahl OS is under active development and is not yet intended for production use.

## Architecture

The repository is split into independent platform components including the
domain core, platform APIs and services, authentication, persistence, and the
rumahl OS frontend.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the system overview and
[`docs/architecture`](docs/architecture/) for detailed architecture documents.

## Current development milestone

The current implementation connects the real platform service and SQLite-backed
local accounts to the React-based rumahl OS shell over HTTPS.

See:

- [Platform service](platform-service/README.md)
- [Buildroot/QEMU image](platform-buildroot/image/README.md)
- [HTTPS acceptance tests](tests/e2e/shell/README.md)

## Development

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
```

Starts the real platform and HTTPS shell with React/CSS hot reload and automatic
Rust rebuilds. See [the developer guide](docs/development.md) for first login,
local certificate trust, WSL/shared folders, and running a prebuilt service on
rumahl OS. Development data stays separate from the installed system.

## Licensing

rumahl is source-available with a layered license model:

- **Source code, contracts, docs, tooling** — Apache License 2.0
  ([`LICENSE`](LICENSE), [`LICENSES/Apache-2.0.txt`](LICENSES/Apache-2.0.txt))
- **First-party artwork** — multi-license, fetched from the separate
  [`rumahl/assets`](https://github.com/rumahl/assets) repository
- **Bundled third-party photos** — Unsplash License
- **Name and logo** — reserved under the [trademark policy](TRADEMARK.md)

Non-code assets live in the central [`assets/`](assets/) store (plus the pinned
`rumahl/assets` checkout) and are copied into the frontend at build time; see
[`assets/README.md`](assets/README.md). The file-to-license mapping is in
[`REUSE.toml`](REUSE.toml), asset origins in [`ASSETS.md`](ASSETS.md), and
contributions in [`CONTRIBUTING.md`](CONTRIBUTING.md).