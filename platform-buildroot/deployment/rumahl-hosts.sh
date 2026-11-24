#!/bin/sh
# Generate the nginx shell-host allowlist and the login address banner from
# /etc/rumahl/platform.env and the current device addresses. Runs before nginx.
set -eu

env_file=/etc/rumahl/platform.env
[ -r "$env_file" ] || exit 0

value() {
    sed -n "s/^$1=//p" "$env_file" | tail -n 1
}

origin="$(value RUMAHL_PUBLIC_ORIGIN)"
[ -n "$origin" ] || exit 0
hosts_field="$(value RUMAHL_PUBLIC_HOSTS)"

authority="${origin#https://}"
authority="${authority%%/*}"
case "$authority" in
    *:*) port_suffix=":${authority##*:}" ;;
    *) port_suffix="" ;;
esac
primary="${authority%%:*}"
case "$authority" in
    \[*\]:*) primary="${authority%:*}" ;;
esac

# Configured host names, plus the primary origin host. The IP token only
# affects the detected-address list, which is always included.
hosts=""
add_host() {
    case " $hosts " in
        *" $1 "*) ;;
        *) hosts="${hosts:+$hosts }$1" ;;
    esac
}
add_host "$primary"
if [ -n "$hosts_field" ]; then
    old_ifs="$IFS"
    IFS=','
    for entry in $hosts_field; do
        entry="$(printf '%s' "$entry" | tr -d '[:space:]')"
        [ -n "$entry" ] || continue
        case "$entry" in
            [Ii][Pp]) continue ;;
        esac
        add_host "$entry"
    done
    IFS="$old_ifs"
fi

# Current global unicast addresses (IPv4 and IPv6).
ips="$(ip -o addr show scope global 2>/dev/null | awk '{print $4}' | cut -d/ -f1 || true)"

mkdir -p /run/nginx /run/nginx/rumahl-apps /run/rumahl
: > /run/rumahl/hosts
for host in $hosts; do
    printf '%s\n' "$host" >> /run/rumahl/hosts
done
: > /run/rumahl/ips
for ip in $ips; do
    printf '%s\n' "$ip" >> /run/rumahl/ips
done

{
    echo 'map $host $rumahl_shell_host_ok {'
    echo '    default 0;'
    for host in $hosts; do
        printf '    %s 1;\n' "$host"
    done
    for ip in $ips; do
        printf '    %s 1;\n' "$ip"
        printf '    [%s] 1;\n' "$ip"
    done
    echo '}'
} > /run/nginx/rumahl-hosts.inc

{
    echo 'rumahl OS'
    echo
    echo 'Device addresses:'
    for host in $hosts; do
        printf '  https://%s%s\n' "$host" "$port_suffix"
    done
    for ip in $ips; do
        case "$ip" in
            *:*) printf '  https://[%s]%s\n' "$ip" "$port_suffix" ;;
            *) printf '  https://%s%s\n' "$ip" "$port_suffix" ;;
        esac
    done
    echo
} > /run/rumahl/issue

# Also print the addresses on the serial console; /etc/issue is owned by
# Buildroot and cannot be replaced with a symlink into /run.
if [ -w /dev/console ]; then
    cat /run/rumahl/issue > /dev/console 2>/dev/null || true
fi
