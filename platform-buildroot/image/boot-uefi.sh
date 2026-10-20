#!/usr/bin/env bash
# Boot the x86_64 UEFI disk image in QEMU/OVMF for development acceptance.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output="${1:-${RUMAHL_BUILDROOT_OUTPUT:-$repo/buildroot/uefi-output}}"
disk="$output/images/rumahl-disk.img"
ovmf_code="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
ovmf_vars="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
test -f "$disk" || { echo "missing $disk; build the UEFI image first" >&2; exit 1; }
test -f "$ovmf_code" || { echo "missing OVMF code: $ovmf_code" >&2; exit 1; }
test -f "$ovmf_vars" || { echo "missing OVMF vars: $ovmf_vars" >&2; exit 1; }
vars="$output/OVMF_VARS.fd"
[ -f "$vars" ] || cp "$ovmf_vars" "$vars"
exec qemu-system-x86_64 -machine q35 -m 2048 -smp 2 -enable-kvm -nographic \
  -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
  -drive "if=pflash,format=raw,file=$vars" \
  -drive "file=$disk,format=raw,if=virtio" \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:8443-:443 \
  -device virtio-net-pci,netdev=net0
