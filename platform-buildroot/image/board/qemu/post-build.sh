#!/bin/sh
set -eu
target="$1"
test -x "$target/usr/bin/rumahl-platform-service"
test -f "$target/usr/share/rumahl/frontend/packages/shell/dist/build-id.json"
# TLS keys are provisioned per device onto the persistent disk, never in firmware.
if [ -L "$target/etc/rumahl/tls" ]; then
    test "$(readlink "$target/etc/rumahl/tls")" = /var/lib/rumahl-tls
else
    test ! -e "$target/etc/rumahl/tls"
    ln -s /var/lib/rumahl-tls "$target/etc/rumahl/tls"
fi
# The QEMU runner exposes guest 443 at host 8443.
sed -i 's|^RUMAHL_PUBLIC_ORIGIN=.*|RUMAHL_PUBLIC_ORIGIN=https://rumahl.home.arpa:8443|' "$target/etc/rumahl/platform.env"
mkdir -p "$target/etc/systemd/system/multi-user.target.wants"
ln -sf /usr/lib/systemd/system/nginx.service "$target/etc/systemd/system/multi-user.target.wants/nginx.service"
