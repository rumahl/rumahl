#!/bin/sh
set -eu
board_dir="$(cd "$(dirname "$0")" && pwd)"
: "${BINARIES_DIR:?BINARIES_DIR is not set}"
cp "$board_dir/grub.cfg" "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
cp "$board_dir/genimage.cfg" "$BINARIES_DIR/genimage.cfg"
support/scripts/genimage.sh -c "$BINARIES_DIR/genimage.cfg"
