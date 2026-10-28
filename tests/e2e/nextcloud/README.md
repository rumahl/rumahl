# Nextcloud OIDC end-to-end harness

This is a **real-container test plan and preflight harness**, not a mock. It is
not run by `cargo test` and intentionally refuses to pull images. A green
preflight does not constitute a successful browser login.

## Prerequisites

- A Linux host with Docker Compose or a compatible Podman Compose installation.
- Preloaded, pinned Nextcloud and MariaDB images. The Nextcloud image must
  already contain the `user_oidc` app; the harness does not fetch apps from a
  network service at test time.
- A live rumahl OS HTTPS issuer reachable both from the browser and Nextcloud,
  with a durable Ed25519 signing key and two independent local OS users.
- Nextcloud installed as a rumahl container app with a confidential OIDC client,
  its recovery-safe secret delivered to the runtime, and the exact callback
  `https://<nextcloud-host>/apps/user_oidc/code` declared in the app manifest.
- A trusted HTTPS reverse proxy for the loopback-bound Nextcloud container.
  Do not disable TLS verification or expose the test database publicly.

The last two points depend on production app-runtime wiring. Until that is
implemented, the harness is preparatory; a green preflight alone is **not** a
successful Nextcloud login.

## Start on a disposable Linux host

Set `NEXTCLOUD_IMAGE`, `MARIADB_IMAGE`, `NEXTCLOUD_DB_ROOT_PASSWORD`,
`NEXTCLOUD_DB_PASSWORD`, `NEXTCLOUD_ADMIN_USER`, `NEXTCLOUD_ADMIN_PASSWORD`,
`NEXTCLOUD_HOST`, `NEXTCLOUD_ORIGIN` and `RUMAHL_ISSUER` in the local shell or a
private environment file. Use test-only passwords and do not commit the file.

```sh
docker compose -f tests/e2e/nextcloud/compose.yaml up -d --pull never
docker compose -f tests/e2e/nextcloud/compose.yaml exec -u www-data nextcloud php occ status
tests/e2e/nextcloud/check.sh
```

Configure `user_oidc` through Nextcloud's admin UI or its `occ
user_oidc:provider` command after checking the installed app's `--help` for
safe secret input. Supply the recovered rumahl client ID and secret; use
`$RUMAHL_ISSUER/.well-known/openid-configuration` as the discovery URI. Do
not paste a secret into a tracked file or a logged shell command. The Nextcloud
project documents this provider command and the callback path in its
[user_oidc README](https://github.com/nextcloud/user_oidc).

## Acceptance sequence

Record the issuer, image digests, client ID, two local user IDs, and the
Nextcloud user IDs in the test report. Never record the client secret, OS
cookies, authorization codes or tokens.

1. Sign in to rumahl OS as local user A, then start Nextcloud's OIDC login in
   that browser. Approve `openid profile` and confirm a Nextcloud account is
   created. Sign out of Nextcloud and the rumahl OS browser session.
2. Repeat in a separate browser profile as local user B. Confirm a *different*
   Nextcloud account and pairwise `sub`. Neither user may access the other's
   Nextcloud files.
3. Restart the rumahl provider and Nextcloud containers **without deleting
   volumes or rotating the signing key**. Repeat both logins; each user must
   return to the same Nextcloud account and the client ID/secret must remain
   unchanged.
4. Log out user A from rumahl OS. Its access token must stop working at
   UserInfo; user B must remain authenticated. Repeat after a provider restart.
5. Revoke/uninstall the Nextcloud client. Pending codes, active access tokens,
   and later token requests must fail. Neither browser may establish a new
   Nextcloud OIDC login.
6. Send malformed requests: wrong redirect URI, missing/wrong S256 verifier,
   replayed code, duplicate form parameter, wrong client secret and token from
   user A used after logout. Confirm rejection without identity disclosure.

Use `docker compose -f tests/e2e/nextcloud/compose.yaml down` to stop the
containers while preserving volumes for recovery investigation. `down -v`
destroys the disposable data and is intentionally not part of this harness.
