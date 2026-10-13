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
# rumahl OS layout: /rumahl is the protected system tree (mode 0700). It holds
# the system services and the admin home /rumahl/root, which replaces /root.
rm -rf "$target/root"
install -d -m 0700 "$target/rumahl" "$target/rumahl/root"
# Bind-mount targets for the data disk's user and app trees (see fstab).
install -d -m 0755 "$target/home" "$target/apps"
mkdir -p "$target/etc/systemd/system/multi-user.target.wants"
ln -sf /usr/lib/systemd/system/nginx.service "$target/etc/systemd/system/multi-user.target.wants/nginx.service"
# The runtime supervisor hosts app containers through Docker. Enable the engine
# and its socket whenever the image ships them.
if [ -e "$target/usr/lib/systemd/system/docker.service" ]; then
    ln -sf /usr/lib/systemd/system/docker.service "$target/etc/systemd/system/multi-user.target.wants/docker.service"
    mkdir -p "$target/etc/systemd/system/sockets.target.wants"
    if [ -e "$target/usr/lib/systemd/system/docker.socket" ]; then
        ln -sf /usr/lib/systemd/system/docker.socket "$target/etc/systemd/system/sockets.target.wants/docker.socket"
    fi
fi
