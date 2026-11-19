#!/usr/bin/env bash
# Builds, signs and installs the built-in development app into the running
# rumahl developer server state.
#
#   1. pnpm --dir frontend dev      # starts the platform service + shell
#   2. pnpm --dir frontend dev:app  # installs the dev app in another terminal
#
# The app then appears in the catalog and on the desktop, and the developer
# account is granted launch access automatically.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

state="${RUMAHL_DEV_STATE:-$repo/.rumahl-dev}"
data="$state/data"
work="$state/app-build"
package="$work/rumahl-dev-app"
keystore="$state/keys/dev-app.json"
trust_store="$state/keys/trust-store.json"

if [ ! -d "$data" ]; then
  echo "Developer state not found at $data." >&2
  echo "Start 'pnpm --dir frontend dev' first, then run this again." >&2
  exit 1
fi

mkdir -p "$work" "$state/keys" "$work/dummy"
rm -rf "$package"
mkdir -p "$package/frontend"

cat > "$package/frontend/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>rumahl Dev App</title>
  <style>body{font:16px system-ui;margin:0;padding:2rem;background:#0f172a;color:#e2e8f0}</style>
  <h1>rumahl Dev App</h1>
  <p>Hello from a signed, installed web app.</p>
</html>
HTML

cat > "$work/manifest.json" <<'JSON'
{
  "formatVersion": 1,
  "publisherId": "com.rumahl",
  "app": {
    "appId": "com.rumahl.devapp",
    "version": "1.0.0",
    "displayName": "rumahl Dev App",
    "runtime": {
      "kind": "web",
      "entrypoints": [
        { "id": "main", "kind": "web-asset", "path": "frontend/index.html" }
      ]
    },
    "settings": [
      { "key": "greeting", "title": "Greeting", "description": "Shown on the start page.", "type": "text" },
      { "key": "enabled", "title": "Enabled", "type": "boolean" }
    ]
  }
}
JSON

echo "Building platform service and package tool..."
cargo build --locked -p rumahl-platform-service -p rumahl-app-packages --bins

if [ ! -f "$keystore" ]; then
  echo "Generating a development signing key..."
  target/debug/rumahl-package keygen --key-id dev-app --publisher com.rumahl \
    --keystore "$keystore" --trust-store "$trust_store"
fi

target/debug/rumahl-package sign \
  --package "$package" \
  --manifest "$work/manifest.json" \
  --keystore "$keystore"

echo "Installing into $data ..."
RUMAHL_STATE_DIR="$data" \
RUMAHL_RUNTIME_UID="$(id -u)" \
RUMAHL_RUNTIME_CONTROL_SOCKET="$work/dummy/control.sock" \
RUMAHL_RUNTIME_SECRET_SOCKET="$work/dummy/secret.sock" \
RUMAHL_RUNTIME_IMAGE_SOCKET="$work/dummy/image.sock" \
RUMAHL_PACKAGE_TRUST_STORE="$trust_store" \
RUMAHL_TPM2_EXECUTABLE=/bin/true \
RUMAHL_SECRET_KEY_ID=dev-root \
RUMAHL_SECRET_KEY_OBJECT="$work/dummy/root.ctx" \
RUMAHL_APP_HOST_SUFFIX=apps.localhost \
RUMAHL_APPS_ROOT="$data/apps" \
target/debug/rumahl-platform-service install-package "$package"

echo "Done. Reload the shell and log in as 'developer'; the app now appears."
