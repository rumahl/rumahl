#!/usr/bin/env bash
# Provision a disposable QEMU development seed: two local accounts, a locally
# trusted development CA, and the persistent ext4 data disk. Development and
# acceptance only. Passwords are development defaults and printed to the seed
# directory; never use this for a real device.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
dev_ca="$repo/platform-buildroot/deployment/dev-ca.sh"
create_data="$repo/platform-buildroot/image/create-data.py"
seed=""
data=""
host="rumahl.home.arpa"
app_suffix="apps.rumahl.home.arpa"
alice_password="${RUMAHL_DEV_ALICE_PASSWORD:-rumahl-Dev-Alice-2026!}"
bob_password="${RUMAHL_DEV_BOB_PASSWORD:-rumahl-Dev-Bob-2026!}"

usage() {
  cat >&2 <<'EOF'
usage: dev-provision.sh --seed DIR --data FILE [--host NAME] [--app-suffix SUFFIX]

  --seed DIR         seed directory representing the device /var
  --data FILE        ext4 data image to create (must not exist)
  --host NAME        shell host name for the certificate (default: rumahl.home.arpa)
  --app-suffix SUFFIX app host wildcard suffix (default: apps.rumahl.home.arpa)

Passwords come from RUMAHL_DEV_ALICE_PASSWORD and RUMAHL_DEV_BOB_PASSWORD.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --seed) seed="$2"; shift 2 ;;
    --data) data="$2"; shift 2 ;;
    --host) host="$2"; shift 2 ;;
    --app-suffix) app_suffix="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; echo "dev-provision.sh: unknown option: $1" >&2; exit 2 ;;
  esac
done

[ -n "$seed" ] || { usage; echo "dev-provision.sh: --seed is required" >&2; exit 2; }
[ -n "$data" ] || { usage; echo "dev-provision.sh: --data is required" >&2; exit 2; }
[ ! -e "$seed" ] || { echo "dev-provision.sh: seed $seed already exists" >&2; exit 2; }

binary="$repo/target/debug/rumahl-platform-service"
if [ ! -x "$binary" ]; then
  echo "==> Building the native provisioning binary"
  (cd "$repo" && cargo build --locked -p rumahl-platform-service)
fi

state="$seed/lib/rumahl"
tls="$seed/lib/rumahl-tls"
mkdir -p "$state" "$tls"
chmod 700 "$seed" "$state" "$tls"

blocklist="$seed/password-blocklist.txt"
cat > "$blocklist" <<'LIST'
password
password123
123456789
letmein
qwerty
LIST

provision() {
  local user="$1" display="$2" password="$3"
  printf '%s' "$password" | \
    RUMAHL_STATE_DIR="$state" \
    RUMAHL_PASSWORD_BLOCKLIST="$blocklist" \
    "$binary" provision-account "$user" "$display" --password-stdin
}
provision alice Alice "$alice_password"
provision bob Bob "$bob_password"

"$dev_ca" --output "$tls" --host "$host" --app-suffix "$app_suffix"

python3 "$create_data" --seed "$seed" --output "$data"

cat > "$seed/credentials.txt" <<CRED
alice  $alice_password
bob    $bob_password
CRED
chmod 600 "$seed/credentials.txt"

echo "Seed ready at $seed; credentials in $seed/credentials.txt"
echo "Data disk ready at $data"
