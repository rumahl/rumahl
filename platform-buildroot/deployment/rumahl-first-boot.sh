#!/bin/sh
# One-time device setup: first local account, device CA, initial TLS certificate
# and persistent state directories. Runs before the platform and nginx.
#
# With /etc/rumahl/first-boot.env present (development and acceptance images)
# the setup runs unattended. Otherwise it prompts on /dev/console.
set -eu

state=/var/lib/rumahl
marker="$state/.provisioned"
log=/var/log/rumahl-first-boot.log
console="${RUMAHL_FIRST_BOOT_CONSOLE:-/dev/tty1}"

[ -e "$marker" ] && exit 0

mkdir -p "$state" /var/lib/rumahl-ca /var/lib/rumahl-tls /var/lib/rumahl-host /apps /home
chmod 700 "$state" /var/lib/rumahl-tls /var/lib/rumahl-host
chmod 755 /var/lib/rumahl-ca
touch "$log"
chmod 644 "$log"
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
# TPM 2.0: seal a random 32-byte secret-store key and persist it at a handle.
# The owner-hierarchy object survives reboots. PCR/measured-boot binding is a
# later hardening step.
secret_handle=0x81000001
tpm_status="not provisioned; secret store stays disabled"
if [ -e /dev/tpmrm0 ] || [ -e /dev/tpm0 ]; then
    tpm_work="$(mktemp -d)"
    if tpm2_createprimary -C o -c "$tpm_work/primary.ctx" >/dev/null 2>&1 \
        && head -c 32 /dev/urandom > "$tpm_work/secret.bin" \
        && tpm2_create -C "$tpm_work/primary.ctx" -u "$tpm_work/sealed.pub" \
            -r "$tpm_work/sealed.priv" -i "$tpm_work/secret.bin" >/dev/null 2>&1 \
        && tpm2_load -C "$tpm_work/primary.ctx" -u "$tpm_work/sealed.pub" \
            -r "$tpm_work/sealed.priv" -c "$tpm_work/sealed.ctx" >/dev/null 2>&1 \
        && tpm2_evictcontrol -C o -c "$tpm_work/sealed.ctx" "$secret_handle" >/dev/null 2>&1 \
        && [ "$(tpm2_unseal --object-context "$secret_handle" 2>/dev/null | wc -c)" -eq 32 ]; then
        tpm_status="provisioned at $secret_handle"
    else
        tpm_status="provisioning failed"
    fi
    rm -rf "$tpm_work"
else
    tpm_status="no TPM device; secret store stays disabled"
fi
echo "TPM sealing: $tpm_status"
printf 'TPM sealing: %s\n' "$tpm_status" >"$console"

# Optional Wi-Fi profile for the first boot; iwd owns the interface and its
# built-in network configuration handles DHCP.
if [ -n "${FIRST_BOOT_WIFI_SSID:-}" ] && [ -n "${FIRST_BOOT_WIFI_PASSWORD:-}" ]; then
    mkdir -p /var/lib/iwd
    printf '[Security]\nPassphrase=%s\n' "$FIRST_BOOT_WIFI_PASSWORD" \
        > "/var/lib/iwd/${FIRST_BOOT_WIFI_SSID}.psk"
    chmod 600 "/var/lib/iwd/${FIRST_BOOT_WIFI_SSID}.psk"
    systemctl restart iwd 2>/dev/null || true
    systemctl restart systemd-networkd 2>/dev/null || true
    echo "Wi-Fi profile written for ${FIRST_BOOT_WIFI_SSID}"
fi
touch "$marker"
chmod 600 "$marker"
echo "=== first boot complete: user=$admin_user host=$hostname ==="
printf '\nFirst-boot setup complete. Open https://<device-ip>/ after the services start.\n' >"$console"
