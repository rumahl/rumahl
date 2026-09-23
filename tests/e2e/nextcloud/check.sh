#!/usr/bin/env bash
set -euo pipefail

: "${RUMAHL_ISSUER:?Set the live HTTPS rumahl issuer origin}"
: "${NEXTCLOUD_ORIGIN:?Set the HTTPS Nextcloud origin}"

for command_name in curl jq; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$command_name" >&2
        exit 2
    fi
done

case "$RUMAHL_ISSUER" in https://*) ;; *) printf 'RUMAHL_ISSUER must be HTTPS\n' >&2; exit 2 ;; esac
case "$NEXTCLOUD_ORIGIN" in https://*) ;; *) printf 'NEXTCLOUD_ORIGIN must be HTTPS\n' >&2; exit 2 ;; esac

discovery="$(curl --fail --silent --show-error --max-time 15 \
    "$RUMAHL_ISSUER/.well-known/openid-configuration")"
printf '%s' "$discovery" | jq --exit-status \
    --arg issuer "$RUMAHL_ISSUER" \
    '.issuer == $issuer
     and (.response_types_supported | index("code") != null)' >/dev/null

token_endpoint="$(printf '%s' "$discovery" | jq -er '.token_endpoint')"
jwks_uri="$(printf '%s' "$discovery" | jq -er '.jwks_uri')"
printf '%s' "$discovery" | jq --exit-status \
    '.code_challenge_methods_supported | index("S256") != null' >/dev/null
printf '%s' "$discovery" | jq --exit-status \
    '.id_token_signing_alg_values_supported | index("EdDSA") != null' >/dev/null
case "$token_endpoint" in "$RUMAHL_ISSUER"/*) ;; *) printf 'Token endpoint is outside issuer\n' >&2; exit 1 ;; esac
case "$jwks_uri" in "$RUMAHL_ISSUER"/*) ;; *) printf 'JWKS endpoint is outside issuer\n' >&2; exit 1 ;; esac

curl --fail --silent --show-error --max-time 15 "$jwks_uri" | jq --exit-status \
    '.keys | any(.[]; .kty == "OKP" and .crv == "Ed25519" and .alg == "EdDSA" and (.kid | length > 0))' >/dev/null
curl --fail --silent --show-error --max-time 15 \
    "$NEXTCLOUD_ORIGIN/status.php" | jq --exit-status '.installed == true' >/dev/null

printf 'Discovery, JWKS, PKCE S256 and Nextcloud installation are reachable.\n'
printf 'Registered Nextcloud callback must be exactly: %s/apps/user_oidc/code\n' "$NEXTCLOUD_ORIGIN"
