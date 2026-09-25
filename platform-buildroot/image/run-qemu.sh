#!/bin/sh
# A read-only system drive and a separately supplied persistent data drive.
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: $0 BUILDROOT_OUTPUT DATA_EXT4" >&2
    exit 2
fi
output=$(realpath "$1")
data=$(realpath "$2")
test -f "$output/images/bzImage"
test -f "$output/images/rootfs.ext2"
test -f "$data"
exec qemu-system-x86_64 -machine q35 -m 2048 -smp 2 -nographic \
    -kernel "$output/images/bzImage" \
    -append "root=/dev/vda ro rootwait console=ttyS0" \
    -drive "file=$output/images/rootfs.ext2,format=raw,if=virtio,readonly=on" \
    -drive "file=$data,format=raw,if=virtio" \
    -netdev user,id=net0,hostfwd=tcp:127.0.0.1:8443-:443 \
    -device virtio-net-pci,netdev=net0
