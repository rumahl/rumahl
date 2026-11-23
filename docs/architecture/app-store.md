# App Store sources

The App Store resolves apps from two source types: a website index and a GitHub
repository. Both are fetched by the platform service, never by the shell, and
both can list any number of apps. Every selected app is downloaded as a signed
directory package and passes the regular package verifier before installation.

## Source configuration

Sources are stored in `$RUMAHL_STATE_DIR/app-store-sources.json` (owner-only).
The shell can add and remove them through:

- `GET /api/v1/shell/store`
- `POST /api/v1/shell/store/sources` with `{ "kind": "website"|"github", "location": "..." }`
- `DELETE /api/v1/shell/store/sources/{id}`
- `POST /api/v1/shell/store/install` with `{ "sourceId", "appId", "device" }`

All routes require a live shell session. Mutations require the exact shell
`Origin`, and installation additionally respects the OS-mode policy.

A **website** location may be the index URL itself (`.../store.json`) or a
directory/base URL, in which case `store.json` is appended. A **GitHub**
location accepts:

```text
owner/repository
github:owner/repository@branch/path/to/store.json
https://github.com/owner/repository
https://github.com/owner/repository/tree/branch/path
https://raw.githubusercontent.com/owner/repository/branch/path
```

The default branch is `main` and the default index is `store.json`. If a GitHub
repository has no index but its root contains `package.json` and
`package.sig.json`, the repository is exposed as a single-app source.

For local testing, `RUMAHL_GITHUB_RAW_BASE` replaces
`https://raw.githubusercontent.com`, so a local server can exercise the GitHub
path without contacting GitHub.

## Index format

Both source types return the same versioned document:

```json
{
  "formatVersion": 1,
  "name": "Example Store",
  "apps": [
    {
      "id": "com.example.notes",
      "title": "Notes",
      "version": "1.0.0",
      "description": "Local notes.",
      "publisher": "com.example",
      "icon": "icons/notes.png",
      "packageUrl": "packages/notes"
    }
  ]
}
```

`id`, `title`, `version` and `packageUrl` are required. `packageUrl` points at a
package directory containing `package.json`, `package.sig.json` and the payload
files named by the signed manifest. Relative URLs resolve against the index
directory. Index metadata is advisory; install re-reads the signed manifest,
requires the requested app ID to match, and returns the installed manifest
metadata. Unsigned packages, unknown publisher keys, changed payloads and
unlisted files fail closed.

## Local example store

The repository ships twenty real example apps under
`examples/app-store/apps`. Generate signed packages and indexes, then serve
them:

```sh
scripts/dev-example-store.sh
```

This writes a generated website index and a GitHub-shaped repository beneath
`.rumahl-dev/example-store/` and starts a loopback server on port 8787:

- Website source: `https://127.0.0.1:8787/site/store.json`
- GitHub source: `example/apps` with
  `RUMAHL_GITHUB_RAW_BASE=https://127.0.0.1:8787/github`

The local server uses a generated development CA
(`.rumahl-dev/example-store-tls/ca.pem`). The developer server passes it to the
platform through `RUMAHL_APP_STORE_CA_FILE`, so store installs trust the local
HTTPS endpoint. The signing key in `.rumahl-dev/keys/trust-store.json` is
trusted the same way.

Public sources must use HTTPS. Plain HTTP is accepted only for loopback hosts
(`localhost`, `127.0.0.1`, `::1`) to keep automated tests and local mirrors
simple, and redirects cannot downgrade to plain HTTP.
