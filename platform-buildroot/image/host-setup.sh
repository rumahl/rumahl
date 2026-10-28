#!/usr/bin/env bash
# Prepare a Debian/Ubuntu build host for the rumahl OS QEMU image. It installs
# the Buildroot host prerequisites, QEMU and swtpm through apt, installs Node
# and pnpm into ~/.local, and works around hosts whose `install` is uutils.
#
# The apt step needs root. Run the whole script with sudo, or copy the printed
# apt command into a root shell.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
node_major=24
pnpm_version=11.19.0

apt_packages=(
  build-essential git cpio unzip rsync bc file wget curl patch gettext
  pkg-config gawk diffutils python3 perl gzip bzip2 tar sed
  libncurses-dev libelf-dev libssl-dev acl attr
  qemu-system-x86 qemu-utils swtpm swtpm-tools
)

if [ "$(id -u)" -eq 0 ]; then
  echo "==> Installing apt packages"
  apt-get update
  apt-get install -y "${apt_packages[@]}"
else
  echo "==> apt packages need root; run this as root (or with sudo):"
  printf 'apt-get update && apt-get install -y'
  printf ' %s' "${apt_packages[@]}"
  printf '\n\n'
fi

# Buildroot rejects the uutils `install`. Prefer GNU install via a user-path
# shim so the system default does not have to change.
if install --version 2>/dev/null | grep -q uutils; then
  gnu="$(command -v gnuinstall || true)"
  if [ -n "$gnu" ]; then
    echo "==> Linking GNU install into ~/.local/bin"
    mkdir -p "$HOME/.local/bin"
    ln -sf "$gnu" "$HOME/.local/bin/install"
    echo "    Ensure ~/.local/bin precedes /usr/bin in PATH for the build."
  else
    echo "WARNING: uutils install detected; install gnu-coreutils or provide gnuinstall" >&2
  fi
fi

# Node and pnpm live in the user's home; they do not need root.
if [ ! -x "$HOME/.local/node/bin/node" ]; then
  echo "==> Installing Node $node_major into ~/.local/node"
  version="$(curl -fsSL https://nodejs.org/dist/index.json | python3 -c "
import json, sys
data = json.load(sys.stdin)
print(next(x['version'] for x in data
           if x['version'].startswith('v${node_major}.') and x['lts']))
")"
  mkdir -p "$HOME/.local/node" "$HOME/.local/bin"
  tmp="$(mktemp -d)"
  curl -fsSL "https://nodejs.org/dist/$version/node-$version-linux-x64.tar.xz" \
    -o "$tmp/node.tar.xz"
  rm -rf "${HOME:?}/.local/node"
  mkdir -p "$HOME/.local/node"
  tar -xJf "$tmp/node.tar.xz" -C "$HOME/.local/node" --strip-components=1
  rm -rf "$tmp"
fi

export PATH="$HOME/.local/node/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

if ! command -v pnpm >/dev/null 2>&1; then
  echo "==> Installing pnpm $pnpm_version"
  npm install -g "pnpm@$pnpm_version" --prefix "$HOME/.local" >/dev/null
fi

echo
echo "==> Installed versions"
node --version
pnpm --version
cargo --version || echo "cargo not found; install rustup from https://rustup.rs"

if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  echo "KVM: accessible"
else
  echo "KVM: not accessible; run as root:"
  echo "  usermod -aG kvm $USER   # then log in again"
  echo "  # or, without re-login:"
  echo "  setfacl -m u:$USER:rw /dev/kvm"
fi

echo
echo "Add this to your shell profile if it is not already there:"
echo "  export PATH=\"\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH\""
echo "Then build and boot:"
echo "  $repo/platform-buildroot/image/build.sh"
echo "  $repo/platform-buildroot/deployment/dev-provision.sh --seed <seed> --data <data>"
echo "  $repo/platform-buildroot/image/boot.sh --seed <seed> --data <data> --tpm"
