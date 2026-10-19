#!/usr/bin/env bash
# Build the rumahl OS Buildroot image for the QEMU x86-64 acceptance target.
#
# Wraps the manual sequence documented in image/README.md:
#   1. fetch and verify the pinned Buildroot source archive
#   2. configure the external tree and the staged-artifacts path
#   3. build the cross toolchain
#   4. cross-compile the Rust services with that toolchain
#   5. build the frontend bundle
#   6. stage target assets (deployment/stage.py)
#   7. build the system image
#
# Run on a Linux build host with the Buildroot host prerequisites
# (https://buildroot.org/downloads/manual/manual.html#requirement-mandatory).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
external="$repo/platform-buildroot/image"
source_json="$external/buildroot-source.json"
target="x86_64-unknown-linux-gnu"

usage() {
  cat >&2 <<'EOF'
usage: build.sh [options]

Options:
  --source DIR      extracted Buildroot source tree
                    (default: $RUMAHL_BUILDROOT_SOURCE or <repo>/buildroot/buildroot-<version>)
  --output DIR      Buildroot output directory
                    (default: $RUMAHL_BUILDROOT_OUTPUT or <repo>/buildroot/output)
  --artifacts DIR   staging directory for target assets, replaced on each run
                    (default: $RUMAHL_ARTIFACTS or <repo>/buildroot/artifacts)
  --download DIR    directory that caches the source archive
                    (default: the parent of --source)
  --jobs N          parallel build jobs (default: $RUMAHL_BUILDROOT_JOBS or nproc)
  -h, --help        show this help
EOF
}

die() {
  echo "build.sh: $*" >&2
  exit 1
}

json_field() {
  python3 - "$source_json" "$1" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)[sys.argv[2]])
PY
}

# Fail before the long toolchain build if a required host tool is missing or if
# the host install is the uutils coreutils that Buildroot rejects.
for tool in cargo rustup node pnpm python3 curl make tar sha256sum realpath; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool not found in PATH: $tool"
done
if install --version 2>/dev/null | grep -q uutils; then
  die "the host 'install' is uutils coreutils; Buildroot requires GNU install (see host-setup.sh)"
fi

version="$(json_field version)"
url="$(json_field url)"
sha256="$(json_field sha256)"
[ -n "$version" ] && [ -n "$url" ] && [ -n "$sha256" ] \
  || die "buildroot-source.json must define version, url and sha256"

source="${RUMAHL_BUILDROOT_SOURCE:-$repo/buildroot/buildroot-$version}"
output="${RUMAHL_BUILDROOT_OUTPUT:-$repo/buildroot/output}"
artifacts="${RUMAHL_ARTIFACTS:-$repo/buildroot/artifacts}"
jobs="${RUMAHL_BUILDROOT_JOBS:-$(nproc)}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --source) source="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    --artifacts) artifacts="$2"; shift 2 ;;
    --download) download="$2"; shift 2 ;;
    --jobs) jobs="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die "unknown option: $1" ;;
  esac
done

download="${download:-$(dirname "$source")}"
source="$(realpath -m "$source")"
output="$(realpath -m "$output")"
artifacts="$(realpath -m "$artifacts")"
download="$(realpath -m "$download")"

case "$artifacts" in
  /|"") die "refusing to use $artifacts as the artifacts directory" ;;
esac
[ -n "$jobs" ] || die "job count must not be empty"

# 1. Fetch, verify and extract the pinned Buildroot source.
if [ ! -d "$source" ]; then
  echo "==> Fetching Buildroot $version"
  mkdir -p "$download" "$(dirname "$source")"
  tarball="$download/buildroot-$version.tar.gz"
  if [ ! -f "$tarball" ]; then
    curl --fail --location "$url" --output "$tarball"
  fi
  echo "$sha256  $tarball" | sha256sum --check --status \
    || die "checksum mismatch for $tarball"
  tar --extract --gzip --file "$tarball" --directory "$download"
  if [ "$source" != "$download/buildroot-$version" ]; then
    mv "$download/buildroot-$version" "$source"
  fi
else
  echo "==> Reusing Buildroot source at $source"
fi

[ -x "$source/utils/config" ] || die "$source is not a Buildroot source tree"

# 2. Configure the external tree and the staged-artifacts path.
echo "==> Configuring $output"
make -C "$source" O="$output" BR2_EXTERNAL="$external" \
  rumahl_qemu_x86_64_defconfig
"$source/utils/config" --file "$output/.config" \
  --set-str BR2_PACKAGE_RUMAHL_SHELL_ARTIFACTS "$artifacts"
make -C "$source" O="$output" olddefconfig

# 3. The Rust services must be built with this image's toolchain.
echo "==> Building cross toolchain (-j$jobs)"
make -C "$source" O="$output" -j"$jobs" toolchain

# 4. Cross-compile the platform service and the runtime supervisor. The C
#    compiler variables let C dependencies (for example SQLite) use the
#    Buildroot toolchain instead of the host toolchain.
echo "==> Cross-compiling Rust services"
rustup target add "$target"
linker="$output/host/bin/x86_64-buildroot-linux-gnu-gcc"
[ -x "$linker" ] || die "expected cross linker at $linker"
export CARGO_TARGET_DIR="$repo/target/buildroot"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$linker"
export CC_x86_64_unknown_linux_gnu="$linker"
cargo build --locked --release --target "$target" -p rumahl-platform-service
cargo build --locked --release --target "$target" \
  -p rumahl-platform-buildroot --bin rumahl-runtime-supervisor

# 5. Build the frontend bundle the image serves.
echo "==> Building frontend"
(cd "$repo/frontend" && pnpm install --frozen-lockfile && pnpm build)

# 6. Stage target assets into a fresh directory. stage.py refuses to overwrite.
echo "==> Staging target assets into $artifacts"
rm -rf "$artifacts"
python3 "$repo/platform-buildroot/deployment/stage.py" \
  --binary "$repo/target/buildroot/$target/release/rumahl-platform-service" \
  --supervisor-binary "$repo/target/buildroot/$target/release/rumahl-runtime-supervisor" \
  --output "$artifacts"

# 7. Build the image. A previously built local package must be cleaned so it
#    picks up the freshly staged assets.
if compgen -G "$output/build/rumahl-shell-*" >/dev/null; then
  echo "==> Cleaning previous rumahl-shell package build"
  make -C "$source" O="$output" rumahl-shell-dirclean
fi
echo "==> Building system image (-j$jobs)"
make -C "$source" O="$output" -j"$jobs"

echo
echo "Kernel: $output/images/bzImage"
echo "Rootfs: $output/images/rootfs.ext2"
echo "Boot with: $repo/platform-buildroot/image/boot.sh --output '$output'"
