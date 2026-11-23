#!/usr/bin/env bash
# Builds and serves twenty signed example apps for the rumahl App Store.
#
#   1. pnpm --dir frontend dev
#   2. scripts/dev-example-store.sh
#   3. Add https://127.0.0.1:8787/site/store.json as a website source.
#
# The output also mirrors the catalog as a GitHub-shaped tree under
# /github/example/apps/main. To exercise the GitHub source path against it,
# set this in the shell console and add "github:example/apps" as a source:
#   globalThis.__RUMAHL_GITHUB_RAW_BASE__ = "https://127.0.0.1:8787/github"
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state="${RUMAHL_DEV_STATE:-$repo/.rumahl-dev}"
# A checkout below /tmp often shares a small tmpfs with node_modules. Keep the
# temporary Cargo build outside the workspace when space is tight.
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  available_kb="$(df -Pk "$repo" | awk 'NR == 2 { print $4 }')"
  if [ "${available_kb:-0}" -lt 524288 ]; then
    CARGO_TARGET_DIR="${HOME}/.cache/rumahl-example-store-target"
    export CARGO_TARGET_DIR
    echo "Low workspace space: building rumahl-package in $CARGO_TARGET_DIR"
  fi
fi
python3 "$repo/scripts/build-example-store.py" --state-dir "$state"
exec python3 "$repo/scripts/example-store-server.py" \
  --root "$state/example-store" --port "${RUMAHL_EXAMPLE_STORE_PORT:-8787}" \
  --tls-cert "$state/example-store-tls/cert.pem" \
  --tls-key "$state/example-store-tls/key.pem"
