#!/usr/bin/env bash
# Generate a development CA and a server certificate for the shell host and its
# per-app wildcard. This is for development and acceptance only; production
# certificates are provisioned per device and never generated here.
#
# Writes ca.pem (install this in the test browser), fullchain.pem and key.pem
# (the pair the platform's TLS directory expects).
set -euo pipefail

host="rumahl.home.arpa"
app_suffix="apps.rumahl.home.arpa"
output=""
days=825

usage() {
  cat >&2 <<'EOF'
usage: dev-ca.sh --output DIR [--host NAME] [--app-suffix SUFFIX] [--days N]

  --output DIR       directory that receives ca.pem, fullchain.pem and key.pem
  --host NAME        shell host name (default: rumahl.home.arpa)
  --app-suffix SUFFIX wildcard suffix for installed apps
                     (default: apps.rumahl.home.arpa)
  --days N           server certificate validity in days (default: 825)
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
  -keyout "$work/ca-key.pem" -out "$work/ca.pem" \
  -subj "/CN=rumahl-dev-ca" >/dev/null 2>&1

cat > "$work/server.cnf" <<EOF
[req]
distinguished_name = dn
req_extensions = req_ext
prompt = no
[dn]
CN = $host
[req_ext]
subjectAltName = @alt
[alt]
DNS.1 = $host
DNS.2 = *.$app_suffix
EOF

openssl req -newkey rsa:2048 -nodes -keyout "$work/key.pem" \
  -out "$work/server.csr" -config "$work/server.cnf" >/dev/null 2>&1

cat > "$work/ext.cnf" <<EOF
subjectAltName = DNS:$host, DNS:*.$app_suffix
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
EOF

openssl x509 -req -in "$work/server.csr" -CA "$work/ca.pem" \
  -CAkey "$work/ca-key.pem" -CAcreateserial -out "$work/server.crt" \
  -days "$days" -sha256 -extfile "$work/ext.cnf" >/dev/null 2>&1

cp "$work/ca.pem" "$output/ca.pem"
cat "$work/server.crt" "$work/ca.pem" > "$output/fullchain.pem"
cp "$work/key.pem" "$output/key.pem"
chmod 600 "$output/key.pem"

echo "Development CA written to $output"
echo "  trust: $output/ca.pem"
echo "  serve: $output/fullchain.pem + $output/key.pem"
openssl x509 -in "$output/fullchain.pem" -noout -subject -ext subjectAltName
