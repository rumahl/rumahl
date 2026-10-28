#!/usr/bin/env bash
# Boot the built image in QEMU with a persistent data disk.
#
# The system disk is read-only; all durable state lives on a separate ext4
# data image. Create that image from a provisioned seed directory (see
# image/README.md) with --seed the first time, then reuse the same file.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output="${RUMAHL_BUILDROOT_OUTPUT:-$repo/buildroot/output}"
data="${RUMAHL_DATA_IMAGE:-$repo/buildroot/rumahl-data.ext4}"
seed=""

usage() {
  cat >&2 <<'EOF'
usage: boot.sh [options]

Options:
  --output DIR   Buildroot output directory
                 (default: $RUMAHL_BUILDROOT_OUTPUT or <repo>/buildroot/output)
  --data FILE    persistent data disk
                 (default: $RUMAHL_DATA_IMAGE or <repo>/buildroot/rumahl-data.ext4)
  --seed DIR     seed directory used to create --data if it does not exist
                 (default: none; an existing data disk is always reused)
  --tpm          start swtpm and attach a TPM 2.0 device (requires swtpm)
  -h, --help     show this help
EOF
}

die() {
  echo "boot.sh: $*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --data) data="$2"; shift 2 ;;
    --seed) seed="$2"; shift 2 ;;
    --tpm) tpm=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die "unknown option: $1" ;;
  esac
done

[ -f "$output/images/bzImage" ] || die "missing $output/images/bzImage; run build.sh first"
[ -f "$output/images/rootfs.ext2" ] || die "missing $output/images/rootfs.ext2; run build.sh first"

if [ ! -f "$data" ]; then
  [ -n "$seed" ] || die "no data disk at $data; provision a seed and pass --seed DIR"
  [ -d "$seed" ] || die "seed directory not found: $seed"
  python3 "$repo/platform-buildroot/image/create-data.py" --seed "$seed" --output "$data"
fi

swtpm_pid=""
cleanup() {
  if [ -n "$swtpm_pid" ]; then
    kill "$swtpm_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if [ "${tpm:-0}" = "1" ]; then
  command -v swtpm >/dev/null 2>&1 || die "--tpm requires swtpm on the host"
  socket="$output/swtpm/swtpm.sock"
  mkdir -p "$output/swtpm"
  rm -f "$socket"
  echo "==> Starting swtpm"
  swtpm socket --tpm2 \
    --tpmstate "dir=$output/swtpm" \
    --ctrl "type=unixio,path=$socket" \
    --log "file=$output/swtpm/swtpm.log" &
  swtpm_pid=$!
  for _ in $(seq 1 50); do
    [ -S "$socket" ] && break
    sleep 0.1
  done
  [ -S "$socket" ] || die "swtpm did not create $socket"
  export RUMAHL_SWTPM_SOCKET="$socket"
fi

[ "${tpm:-0}" = "1" ] || echo "==> Attach a TPM with --tpm to exercise the sealed secret store"
"$repo/platform-buildroot/image/run-qemu.sh" "$output" "$data"
