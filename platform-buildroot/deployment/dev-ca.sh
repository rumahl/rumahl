#!/usr/bin/env bash
# Generate a development CA and a server certificate for one or more shell
# hosts and their per-app wildcards. This is for development and acceptance
# only; production certificates are provisioned per device.
#
# Writes ca.pem (install this in the test browser), fullchain.pem and key.pem
# (the pair the platform's TLS directory expects). ca-key.pem is written too,
# because a device-local CA signs the IP-SAN certificate at boot.
set -euo pipefail

hosts=""
host=""
app_suffix=""
output=""
days=825

usage() {
  cat >&2 <<'EOF'
usage: dev-ca.sh --output DIR [--hosts NAME,NAME] [--host NAME]
                 [--app-suffix SUFFIX] [--days N]

  --output DIR       directory that receives ca.pem, ca-key.pem, fullchain.pem
                     and key.pem
  --hosts LIST       comma-separated shell host names; each gets a
                     *.apps.<name> wildcard
  --host NAME        additional single host name
  --app-suffix SUFFIX wildcard suffix for a single host (legacy form)
  --days N           server certificate validity in days (default: 825)
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --hosts) hosts="$2"; shift 2 ;;
    --host) host="$2"; shift 2 ;;
    --app-suffix) app_suffix="$2"; shift 2 ;;
    --days) days="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; echo "dev-ca.sh: unknown option: $1" >&2; exit 2 ;;
  esac
done

[ -n "$output" ] || { usage; echo "dev-ca.sh: --output is required" >&2; exit 2; }
command -v openssl >/dev/null 2>&1 || { echo "dev-ca.sh: openssl is required" >&2; exit 1; }
mkdir -p "$output"
output="$(cd "$output" && pwd)"

names=()
if [ -n "$hosts" ]; then
  old_ifs="$IFS"
  IFS=','
  for name in $hosts; do
    name="$(printf '%s' "$name" | tr -d '[:space:]')"
    [ -n "$name" ] && names+=("$name")
  done
  IFS="$old_ifs"
fi
[ -n "$host" ] && names+=("$host")
[ "${#names[@]}" -gt 0 ] || names=("rumahl.home.arpa")

primary="${names[0]}"
sans=()
for name in "${names[@]}"; do
  sans+=("DNS:$name")
done
if [ -n "$app_suffix" ] && [ "${#names[@]}" -eq 1 ]; then
  sans+=("DNS:*.$app_suffix")
else
  for name in "${names[@]}"; do
    sans+=("DNS:*.apps.$name")
  done
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Reuse an existing development CA so browser trust survives regeneration.
if [ -f "$output/ca.pem" ] && [ -f "$output/ca-key.pem" ]; then
  cp "$output/ca.pem" "$work/ca.pem"
  cp "$output/ca-key.pem" "$work/ca-key.pem"
else
  openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
    -keyout "$work/ca-key.pem" -out "$work/ca.pem" \
    -subj "/CN=rumahl-dev-ca" >/dev/null 2>&1
fi

{
  echo '[req]'
  echo 'distinguished_name = dn'
  echo 'req_extensions = req_ext'
  echo 'prompt = no'
  echo '[dn]'
  echo "CN = $primary"
  echo '[req_ext]'
  echo 'subjectAltName = @alt'
  echo '[alt]'
  index=1
  for san in "${sans[@]}"; do
    echo "DNS.$index = ${san#DNS:}"
    index=$((index + 1))
  done
} > "$work/server.cnf"

openssl req -newkey rsa:2048 -nodes -keyout "$work/key.pem" \
  -out "$work/server.csr" -config "$work/server.cnf" >/dev/null 2>&1

{
  printf 'subjectAltName = '
  separator=''
  for san in "${sans[@]}"; do
    printf '%s%s' "$separator" "$san"
    separator=', '
  done
  echo
  echo 'basicConstraints = CA:FALSE'
  echo 'keyUsage = digitalSignature, keyEncipherment'
  echo 'extendedKeyUsage = serverAuth'
} > "$work/ext.cnf"

openssl x509 -req -in "$work/server.csr" -CA "$work/ca.pem" \
  -CAkey "$work/ca-key.pem" -CAcreateserial -out "$work/server.crt" \
  -days "$days" -sha256 -extfile "$work/ext.cnf" >/dev/null 2>&1

cp "$work/ca.pem" "$output/ca.pem"
cp "$work/ca-key.pem" "$output/ca-key.pem"
chmod 600 "$output/ca-key.pem"
cat "$work/server.crt" "$work/ca.pem" > "$output/fullchain.pem"
cp "$work/key.pem" "$output/key.pem"
chmod 600 "$output/key.pem"

echo "Development CA written to $output"
echo "  trust: $output/ca.pem"
echo "  serve: $output/fullchain.pem + $output/key.pem"
openssl x509 -in "$output/fullchain.pem" -noout -subject -ext subjectAltName
