#!/bin/sh
set -eu
target="$1"
test -x "$target/usr/bin/rumahl-platform-service"
test -f "$target/usr/share/rumahl/frontend/packages/shell/dist/build-id.json"
# TLS lives on the persistent data partition.
if [ -L "$target/etc/rumahl/tls" ]; then
    test "$(readlink "$target/etc/rumahl/tls")" = /var/lib/rumahl-tls
else
    test ! -e "$target/etc/rumahl/tls"
    ln -s /var/lib/rumahl-tls "$target/etc/rumahl/tls"
fi
# rumahl OS layout: protected system tree and bind-mount targets for /home and
# /apps from the data partition.
rm -rf "$target/root"
install -d -m 0700 "$target/rumahl" "$target/rumahl/root"
install -d -m 0755 "$target/home" "$target/apps"
mkdir -p "$target/etc/systemd/system/multi-user.target.wants"
ln -sf /usr/lib/systemd/system/nginx.service "$target/etc/systemd/system/multi-user.target.wants/nginx.service"
if [ -e "$target/usr/lib/systemd/system/docker.service" ]; then
    ln -sf /usr/lib/systemd/system/docker.service "$target/etc/systemd/system/multi-user.target.wants/docker.service"
    mkdir -p "$target/etc/systemd/system/sockets.target.wants"
    if [ -e "$target/usr/lib/systemd/system/docker.socket" ]; then
        ln -sf /usr/lib/systemd/system/docker.socket "$target/etc/systemd/system/sockets.target.wants/docker.socket"
    fi
fi
if [ -e "$target/usr/lib/systemd/system/systemd-networkd-wait-online.service" ]; then
    mkdir -p "$target/etc/systemd/system/network-online.target.wants"
    ln -sf /usr/lib/systemd/system/systemd-networkd-wait-online.service "$target/etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service"
fi
# mDNS and Wi-Fi management.
for unit in avahi-daemon.service iwd.service; do
    if [ -e "$target/usr/lib/systemd/system/$unit" ]; then
        ln -sf "/usr/lib/systemd/system/$unit" "$target/etc/systemd/system/multi-user.target.wants/$unit"
    fi
done
if [ -e "$target/usr/lib/systemd/system/avahi-daemon.socket" ]; then
    mkdir -p "$target/etc/systemd/system/sockets.target.wants"
    ln -sf /usr/lib/systemd/system/avahi-daemon.socket "$target/etc/systemd/system/sockets.target.wants/avahi-daemon.socket"
fi

# Headless installations need a serial login after first-boot setup.
mkdir -p "$target/etc/systemd/system/getty.target.wants"
ln -sf /usr/lib/systemd/system/serial-getty@.service "$target/etc/systemd/system/getty.target.wants/serial-getty@ttyS0.service"
