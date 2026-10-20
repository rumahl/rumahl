#!/usr/bin/env bash
# Boot the x86_64 UEFI disk image in QEMU/OVMF for development acceptance.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output=""
tpm=0

usage() {
  cat >&2 <<'USAGE'
usage: boot-uefi.sh [--output DIR] [--tpm]

  --output DIR  Buildroot output directory containing images/rumahl-disk.img
  --tpm         start swtpm and attach a TPM 2.0 device (requires swtpm)
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --tpm) tpm=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) usage; exit 2 ;;
    *) output="$1"; shift ;;
  esac
done
output="${output:-${RUMAHL_BUILDROOT_OUTPUT:-$repo/buildroot/uefi-output}}"
disk="$output/images/rumahl-disk.img"
ovmf_code="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
ovmf_vars="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
test -f "$disk" || { echo "missing $disk; build the UEFI image first" >&2; exit 1; }
test -f "$ovmf_code" || { echo "missing OVMF code: $ovmf_code" >&2; exit 1; }
test -f "$ovmf_vars" || { echo "missing OVMF vars: $ovmf_vars" >&2; exit 1; }
vars="$output/OVMF_VARS.fd"
[ -f "$vars" ] || cp "$ovmf_vars" "$vars"

swtpm_pid=""
cleanup() {
  [ -n "$swtpm_pid" ] && kill "$swtpm_pid" 2>/dev/null || true
}
trap cleanup EXIT
tpm_args=()
if [ "$tpm" = "1" ]; then
  command -v swtpm >/dev/null 2>&1 || { echo "--tpm requires swtpm" >&2; exit 1; }
  socket="$output/swtpm/swtpm.sock"
  mkdir -p "$output/swtpm"
  rm -f "$socket"
  swtpm socket --tpm2 --tpmstate "dir=$output/swtpm" \
    --ctrl "type=unixio,path=$socket" --log "file=$output/swtpm/swtpm.log" &
  swtpm_pid=$!
  for _ in $(seq 1 50); do
    [ -S "$socket" ] && break
    sleep 0.1
  done
  [ -S "$socket" ] || { echo "swtpm did not create $socket" >&2; exit 1; }
  tpm_args=(-chardev "socket,id=chrtpm,path=$socket" \
    -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-tis,tpmdev=tpm0)
fi

qemu-system-x86_64 -machine q35 -m 2048 -smp 2 -enable-kvm -nographic \
  -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
  -drive "if=pflash,format=raw,file=$vars" \
  -drive "file=$disk,format=raw,if=virtio" \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:8443-:443 \
  -device virtio-net-pci,netdev=net0 \
  "${tpm_args[@]}"
