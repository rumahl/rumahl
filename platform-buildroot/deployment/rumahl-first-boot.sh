#!/bin/sh
# One-time device setup: first local account, device CA, initial TLS certificate
# and persistent state directories. Runs before the platform and nginx.
#
# With /etc/rumahl/first-boot.env present (development and acceptance images)
# the setup runs unattended. Otherwise it prompts on /dev/console.
set -eu

state=/var/lib/rumahl
marker="$state/.provisioned"
log="$state/first-boot.log"
console=/dev/console

[ -e "$marker" ] && exit 0

mkdir -p "$state" /var/lib/rumahl-ca /var/lib/rumahl-tls /var/lib/rumahl-host /apps /home
chmod 700 "$state" /var/lib/rumahl-ca /var/lib/rumahl-tls /var/lib/rumahl-host
exec >>"$log" 2>&1
echo "=== rumahl first boot $(date) ==="

admin_user=admin
admin_display=Administrator
admin_password=
hostname=rumahl
env_file=/etc/rumahl/first-boot.env
if [ -r "$env_file" ]; then
    # shellcheck disable=SC1090
    . "$env_file"
    admin_user="${FIRST_BOOT_USER:-$admin_user}"
    admin_display="${FIRST_BOOT_DISPLAY:-$admin_display}"
    admin_password="${FIRST_BOOT_PASSWORD:-}"
    hostname="${FIRST_BOOT_HOSTNAME:-$hostname}"
fi

if [ -z "$admin_password" ]; then
    if [ ! -t 0 ]; then
        echo "no console and no $env_file; leaving the device unprovisioned"
        exit 0
    fi
    printf '\n\nrumahl OS first-boot setup\n\n' >"$console"
    printf 'Admin username [%s]: ' "$admin_user" >"$console"
    read -r value <"$console" || value=
    [ -n "${value:-}" ] && admin_user="$value"
    printf 'Display name [%s]: ' "$admin_display" >"$console"
    read -r value <"$console" || value=
    [ -n "${value:-}" ] && admin_display="$value"
    printf 'Hostname [%s]: ' "$hostname" >"$console"
    read -r value <"$console" || value=
    [ -n "${value:-}" ] && hostname="$value"
    while :; do
        printf 'Admin password (min 15 characters): ' >"$console"
        stty -echo <"$console"
        read -r first <"$console" || first=
        stty echo <"$console"
        printf '\nRepeat password: ' >"$console"
        stty -echo <"$console"
        read -r second <"$console" || second=
        stty echo <"$console"
        printf '\n' >"$console"
        [ "$first" = "$second" ] || { echo 'Passwords do not match.' >"$console"; continue; }
        [ ${#first} -ge 15 ] || { echo 'Password is too short.' >"$console"; continue; }
        admin_password="$first"
        break
    done
fi

cat > "$state/password-blocklist.txt" <<'LIST'
password
password123
123456789
letmein
qwerty
LIST
chmod 600 "$state/password-blocklist.txt"

# Device CA. The host-config broker later re-signs the server certificate with
# the configured host names and the current IP SANs.
if [ ! -f /var/lib/rumahl-ca/ca.pem ] || [ ! -f /var/lib/rumahl-ca/ca-key.pem ]; then
    openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
        -keyout /var/lib/rumahl-ca/ca-key.pem -out /var/lib/rumahl-ca/ca.pem \
        -subj "/CN=rumahl-device-ca" >/dev/null 2>&1
    chmod 600 /var/lib/rumahl-ca/ca-key.pem
    chmod 644 /var/lib/rumahl-ca/ca.pem
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cat > "$work/server.cnf" <<CONF
[req]
distinguished_name = dn
req_extensions = req_ext
prompt = no
[dn]
CN = $hostname
[req_ext]
subjectAltName = @alt
[alt]
DNS.1 = $hostname
DNS.2 = $hostname.local
CONF
openssl req -newkey rsa:2048 -nodes -keyout "$work/key.pem" \
    -out "$work/server.csr" -config "$work/server.cnf" >/dev/null 2>&1
cat > "$work/ext.cnf" <<CONF
subjectAltName = DNS:$hostname,DNS:$hostname.local
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
CONF
openssl x509 -req -in "$work/server.csr" -CA /var/lib/rumahl-ca/ca.pem \
    -CAkey /var/lib/rumahl-ca/ca-key.pem -CAcreateserial \
    -out "$work/server.crt" -days 825 -sha256 -extfile "$work/ext.cnf" >/dev/null 2>&1
cat "$work/server.crt" /var/lib/rumahl-ca/ca.pem > /var/lib/rumahl-tls/fullchain.pem
cp "$work/key.pem" /var/lib/rumahl-tls/key.pem
chmod 600 /var/lib/rumahl-tls/fullchain.pem /var/lib/rumahl-tls/key.pem

printf '%s' "$admin_password" | \
    RUMAHL_STATE_DIR="$state" \
    RUMAHL_PASSWORD_BLOCKLIST="$state/password-blocklist.txt" \
    /usr/bin/rumahl-platform-service provision-account "$admin_user" "$admin_display" --password-stdin

printf '%s\n' "$hostname" > "$state/hostname"
chmod 600 "$state/hostname"
if [ -f /etc/rumahl/package-trust-store.json ]; then
    echo "package trust store: present"
else
    echo "package trust store: missing; app installation stays disabled"
fi
echo "TPM sealing: not provisioned (secret store fails closed)"
touch "$marker"
chmod 600 "$marker"
echo "=== first boot complete: user=$admin_user host=$hostname ==="
printf '\nFirst-boot setup complete. Open https://<device-ip>/ after the services start.\n' >"$console"
