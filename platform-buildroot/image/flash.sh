#!/usr/bin/env bash
# Write the rumahl UEFI disk image to a block device. This ERASES the target.
# Run as root (for example with sudo) from the build host.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
image=""
device=""
grow=0
yes=0

usage() {
  cat >&2 <<EOF
usage: flash.sh IMAGE DEVICE [--grow] [--yes]

  IMAGE    disk image produced by build.sh (images/rumahl-disk.img)
  DEVICE   whole block device to overwrite, e.g. /dev/sdb (NOT a partition)
  --grow   grow partition 3 and its filesystem to fill the device
  --yes    skip the confirmation prompt

This is destructive. The device must not have mounted partitions.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --grow) grow=1; shift ;;
    --yes) yes=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) usage; exit 2 ;;
    *)
      if [ -z "$image" ]; then
        image="$1"
      elif [ -z "$device" ]; then
        device="$1"
      else
        usage
        exit 2
      fi
      shift
      ;;
  esac
done
[ -n "$image" ] && [ -n "$device" ] || { usage; exit 2; }
[ "$(id -u)" -eq 0 ] || { echo "flash.sh: run as root (sudo)" >&2; exit 1; }
[ -f "$image" ] || { echo "flash.sh: image not found: $image" >&2; exit 1; }
[ -b "$device" ] || { echo "flash.sh: not a block device: $device" >&2; exit 1; }
[ "$(lsblk -ndo TYPE "$device")" = "disk" ] \
  || { echo "flash.sh: pass the whole disk, not a partition: $device" >&2; exit 1; }
if lsblk -nlo MOUNTPOINT "$device" | grep -q .; then
  echo "flash.sh: unmount every partition of $device first" >&2
  lsblk -o NAME,SIZE,TYPE,MOUNTPOINT "$device" >&2
  exit 1
fi

echo "Target device:"
lsblk -o NAME,SIZE,TYPE,MODEL,MOUNTPOINT "$device"
echo
echo "Image: $image ($(ls -lh "$image" | awk '{print $5}'))"
echo "SHA-256: $(sha256sum "$image" | awk '{print $1}')"
if [ "$yes" -ne 1 ]; then
  printf 'Type ERASE to overwrite %s: ' "$device" >&2
  read -r confirm
  [ "$confirm" = "ERASE" ] || { echo "aborted" >&2; exit 1; }
fi

dd if="$image" of="$device" bs=4M conv=fsync status=progress
sync

if [ "$grow" -eq 1 ]; then
  command -v parted >/dev/null 2>&1 || { echo "flash.sh: parted is required for --grow" >&2; exit 1; }
  echo "Growing partition 3 to fill $device"
  parted -s "$device" resizepart 3 100%
  partprobe "$device" 2>/dev/null || true
  data="$(lsblk -nlo PATH "$device" | tail -n 1)"
  [ -b "$data" ] || { echo "flash.sh: could not resolve the data partition" >&2; exit 1; }
  e2fsck -f -y "$data" || true
  resize2fs "$data"
fi

sync
echo
echo "Done. Boot the device in UEFI mode (Secure Boot off for now)."
echo "Acceptance checklist: docs/architecture/real-hardware-acceptance.md"
echo "Host-side checks:     python3 tests/e2e/hardware/check.py --origin https://<device-ip>"
