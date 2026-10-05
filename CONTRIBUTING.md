# Contributing to rumahl OS

Thanks for your interest in rumahl. This guide covers development setup, the
conventions this repository follows, how changes are reviewed, and how
contributions are licensed.

By participating you agree to follow this guide and the
[AI policy](AI_POLICY.md).

## Ways to contribute

- **Bug reports and proposals** — open an issue with a clear reproduction or a
  concrete rationale.
- **Code** — fix bugs, implement features, improve tests.
- **Documentation** — `README.md`, `ARCHITECTURE.md`, `docs/`.
- **Artwork** — belongs in [`rumahl/assets`](https://github.com/rumahl/assets),
  not in this repository (see [Licensing](#licensing)).

## Development setup

See the [developer guide](docs/development.md) for the real Rust platform,
SQLite accounts, React SSR/Vite and local HTTPS. Requirements: Linux or WSL2,
the pinned Rust toolchain (see `rust-toolchain.toml`), Node 22.12+ (or Node 24),
pnpm 11.19.0, and OpenSSL.

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
```

## Architecture principles

rumahl OS is a local-first platform. Before changing code, read
[ARCHITECTURE.md](ARCHITECTURE.md). The platform-wide invariants matter more
than any single feature:

- Core provides mechanisms, not products; `core/` must not contain app-specific
  behavior.
- Apps depend on capabilities, not concrete providers.
- Requesting a permission is not a grant.
- First-party apps use the same platform contracts as third-party apps.
- Resources are logical references, never host paths.
- Transport, persistence, authentication and UI concerns stay outside the domain
  core.
- Privileged OS operations are performed by isolated services.

Keep dependencies pointing toward the abstractions, not from them.

## Making changes

### Branches

Use a short, typed branch name:

```
feat/<topic>   fix/<topic>   docs/<topic>   ci/<topic>   test/<topic>
refactor/<topic>   perf/<topic>   build/<topic>   chore/<topic>   security/<topic>
```

### Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/):
`<type>(<scope>): <summary>`. The types used here are `feat`, `fix`,
`refactor`, `perf`, `test`, `docs`, `ci`, `chore`, `build` and `security`.
Write the summary in the imperative mood without a trailing period. Keep each
commit focused; maintainers may ask to split an oversized change.

### Sign-off

Every commit must be signed off (`git commit -s`). See
[Developer Certificate of Origin](#developer-certificate-of-origin). The `dco`
CI job rejects a pull request if any of its commits is missing the sign-off.

## Checks

Run the checks for the areas you touched before pushing:

```sh
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

# Frontend
pnpm --dir frontend lint
pnpm --dir frontend typecheck
pnpm --dir frontend test
pnpm --dir frontend build

# Licensing
reuse lint
```

CI additionally runs dependency, license (`reuse`), DCO and workflow checks.

## Tests

Add or update tests for behavior changes where practical.

- Rust: unit tests next to the code, integration tests under `*/tests/`.
- Frontend: `*.test.ts` / `*.test.tsx` files colocated with the code.

Prefer exercising behavior through the platform contracts rather than internal
details.

## Dependencies

- **Rust** — `Cargo.lock` is committed. The dependency policy is in
  [`deny.toml`](deny.toml): wildcard version requirements are denied and new
  dependencies must match the license allowlist.
- **Frontend** — `pnpm-lock.yaml` is committed and dependencies are pinned to
  exact versions.
- Do not add a dependency for something trivial. Discuss larger additions in the
  issue or pull request first.

## Generated and fetched directories

Do not edit or commit generated or fetched content; it is gitignored and
recreated by the build:

- `frontend/packages/shell/src/assets/` (copied from the asset store)
- `assets/upstream/` (pinned `rumahl/assets` checkout)
- `frontend/packages/*/dist/`, `target/`

## Working with AI

Read the [AI policy](AI_POLICY.md). In short: AI tools are allowed and treated
as ordinary development tools, but you stay accountable for everything you
submit, you must understand and be able to explain your changes, and autonomous
agents must not open pull requests or issues, or respond to reviews, without
explicit permission.

## Licensing

rumahl uses [SPDX](https://spdx.dev/) / [REUSE](https://reuse.software/) so that
**every tracked project file has a declared license**, determined by its path.
That is what makes a multi-license repository workable: there is exactly one
answer to "what license is this file under?", and it is machine-checkable.

| Layer | License | Scope |
| --- | --- | --- |
| Source code, contracts, docs, tooling | Apache-2.0 | `core/`, `platform-*`, `account-auth/`, `oidc-provider/`, `app-*`, `persistence-sqlite/`, `ui-contracts/`, `frontend/`, `docs/`, build tooling |
| Bundled third-party artwork | declared per path upstream (currently `LicenseRef-Unsplash`) | [`rumahl/assets`](https://github.com/rumahl/assets), fetched into gitignored `assets/upstream/` |
| First-party artwork | multi-license, declared upstream (default CC BY-NC-SA 4.0) | [`rumahl/assets`](https://github.com/rumahl/assets), fetched into gitignored `assets/upstream/` |
| Name and logo | Trademark policy | see [TRADEMARK.md](TRADEMARK.md) |

The path-to-license mapping lives in [`REUSE.toml`](REUSE.toml); the license
texts live in [`LICENSES/`](LICENSES/). Generated and fetched directories
(`frontend/packages/shell/src/assets/`, `assets/upstream/`) are gitignored and
therefore outside REUSE's scope — their terms travel with their source.

### Multi-license, the short version

1. **The path decides the license.** No per-file guessing.
2. **Declare once per tree** in `REUSE.toml` (path globs), or **per file** with a
   `<file>.license` sidecar for binaries and images, which cannot carry a header.
3. **Adding a license is a data change**, not a policy rewrite: drop
   `LICENSES/<SPDX-id>.txt`, add the annotation, add a row to
   [`ASSETS.md`](ASSETS.md). Use `LicenseRef-<name>.txt` for licenses that are
   not on the SPDX list.
4. **SPDX expressions compose.** If a file genuinely needs more than one
   license, express it (`A AND B`, `A OR B`) rather than inventing a new rule.
5. **`reuse lint` verifies** that every covered file carries a copyright notice
   and a valid SPDX license expression.

### Contributing artwork

- **Never relicense third-party assets.** Record the real license and
  attribution; do not apply the project's terms to someone else's work.
- **Standalone artwork files** (wallpapers, images, exported icons and other
  binary art) belong in [`rumahl/assets`](https://github.com/rumahl/assets), not
  here. That repository is multi-license and owns the `REUSE.toml` / `LICENSES/`
  declarations; each of its subtrees declares its license, and those files
  travel with the pinned checkout. This repository only fetches and copies them.
- **UI assets that are part of the application itself** follow the same rule
  unless explicitly documented otherwise:
  - icons and the logomark defined in code (SVG components such as
    `frontend/packages/shell/src/icons.tsx` and `RumahlMark.tsx`) are source code
    and stay here under Apache-2.0;
  - exported or otherwise standalone art goes to `rumahl/assets`.
  When in doubt, ask in the issue.
- When the asset set changes, update [`ASSETS.md`](ASSETS.md) and the pin in
  `assets/upstream.lock.json`. Distribution notices (for the OS image) can be
  generated from the REUSE metadata.

### Developer Certificate of Origin

All commits must be signed off under the [Developer Certificate of Origin
1.1](https://developercertificate.org/):

```sh
git commit -s
```

The sign-off confirms that you have the right to submit the contribution under
**the license already applicable to the paths you changed**. You cannot use a
contribution to relicense a path or a tree; changing a declared license is a
maintainer decision.

There is currently **no Contributor License Agreement (CLA)**. Apache-2.0
contributions cannot later be relicensed without your agreement. If the project
ever needs to offer proprietary or differently licensed terms for code that
incorporates your contribution, the maintainers will ask you separately. Do not
assume a relicensing right because you signed off.

### Licensing checklist

- [ ] Every new file falls under an existing layer, or you added an annotation
      plus a `LICENSES/` text for a new license.
- [ ] `ASSETS.md` updated for every new or changed asset.
- [ ] Third-party assets have a source URL and their real license recorded.
- [ ] `reuse lint` passes (if the REUSE tool is installed).

## Opening a pull request

- Keep one logical change per pull request and explain **what** and **why**;
  link related issues.
- Make sure every commit is signed off and all checks pass.
- Expect review. Answer questions in your own words — see the
  [AI policy](AI_POLICY.md).
- Avoid unrelated refactors and reformatting in the same pull request.
- Maintainers merge; do not force-push over review comments without
  explanation.

Keep changes focused, match existing conventions, and never add a file that
contradicts its declaration in `REUSE.toml`.
