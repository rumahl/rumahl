#!/bin/sh
set -eu
board_dir="$(cd "$(dirname "$0")" && pwd)"
: "${BINARIES_DIR:?BINARIES_DIR is not set}"
# Use the root filesystem UUID as the GPT partition UUID so the kernel can
# resolve root=PARTUUID without an initramfs.
uuid=$(dumpe2fs "$BINARIES_DIR/rootfs.ext2" 2>/dev/null | sed -n 's/^Filesystem UUID: *\(.*\)/\1/p')
[ -n "$uuid" ] || { echo "post-image: rootfs UUID is missing" >&2; exit 1; }
sed "s/UUID_TMP/$uuid/g" "$board_dir/grub.cfg" > "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
sed "s/UUID_TMP/$uuid/g" "$board_dir/genimage.cfg" > "$BINARIES_DIR/genimage.cfg"
support/scripts/genimage.sh -c "$BINARIES_DIR/genimage.cfg"
