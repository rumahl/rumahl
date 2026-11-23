#!/usr/bin/env bash
# One-command QEMU acceptance for the rumahl OS image.
#
#   platform-buildroot/image/acceptance.sh [--skip-build] [--fresh] [--tpm]
#                                          [--keep-vm] [--jobs N]
#
# Runs the full local pipeline automatically:
#   0. preflight: host tools, free disk, hosts entry, free port 8443
#   1. build the QEMU image (build.sh, with host ccache when available)
#   2. provision a dev seed: alice/bob accounts, dev CA, ext4 data disk
#   3. boot the image in QEMU in the background
#   4. run the HTTPS acceptance checks (tests/e2e/hardware/check.py)
#   5. shut the VM down and report PASS/FAIL
#
# Everything lands under <repo>/buildroot/acceptance/ and is reused on the next
# run: the Buildroot output tree, the ccache, the seed and the data disk.
# Development and acceptance only; the passwords are development defaults.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
image="$repo/platform-buildroot/image"
state="${RUMAHL_ACCEPTANCE_DIR:-$repo/buildroot/acceptance}"
output="${RUMAHL_BUILDROOT_OUTPUT:-$state/output}"
seed="$state/seed"
data="$state/rumahl-data.ext4"
log="$state/qemu.log"
pid_file="$state/qemu.pid"
origin_host="rumahl.home.arpa"
host_port="8443"
jobs="${RUMAHL_BUILDROOT_JOBS:-$(nproc)}"
skip_build=0
fresh=0
tpm=0
keep_vm=0
vm_pid=""

usage() {
  cat >&2 <<EOF
usage: acceptance.sh [options]

Options:
  --skip-build   reuse the existing Buildroot output tree (fails if absent)
  --fresh        delete seed and data disk before provisioning
  --tpm          attach a swtpm TPM 2.0 device to the VM
  --keep-vm      leave the VM running after the checks (for manual poking)
  --jobs N       parallel build jobs (default: nproc)
  -h, --help     show this help

Environment: RUMAHL_ACCEPTANCE_DIR (default <repo>/buildroot/acceptance),
RUMAHL_BUILDROOT_OUTPUT, RUMAHL_DEV_ALICE_PASSWORD/RUMAHL_DEV_BOB_PASSWORD
(see deployment/dev-provision.sh).
EOF
}

die() { echo "acceptance.sh: $*" >&2; exit 1; }
note() { echo "==> $*"; }

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-build) skip_build=1; shift ;;
    --fresh) fresh=1; shift ;;
    --tpm) tpm=1; shift ;;
    --keep-vm) keep_vm=1; shift ;;
    --jobs) jobs="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die "unknown option: $1" ;;
  esac
done

mkdir -p "$state"

# ---- 0. Preflight ---------------------------------------------------------
for tool in cargo rustup node pnpm python3 curl make tar sha256sum realpath qemu-system-x86_64; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool not found in PATH: $tool (run image/host-setup.sh)"
done
if install --version 2>/dev/null | grep -q uutils; then
  die "the host 'install' is uutils coreutils; Buildroot requires GNU install (see host-setup.sh)"
fi
if [ "$tpm" = 1 ] && ! command -v swtpm >/dev/null 2>&1; then
  die "--tpm requires swtpm on the host (see host-setup.sh)"
fi

free_kb=$(df -Pk "$state" | awk 'NR==2 {print $4}')
required_kb=$((20 * 1024 * 1024))
if [ "$free_kb" -lt "$required_kb" ]; then
  die "only $((free_kb / 1024 / 1024)) GiB free at $state; the Buildroot tree needs >= 20 GiB"
fi

if ! getent hosts "$origin_host" | grep -q '127\.0\.0\.1'; then
  cat >&2 <<EOF
acceptance.sh: $origin_host does not resolve to 127.0.0.1 on this host.
Fix it once with:
  echo '127.0.0.1 $origin_host' | sudo tee -a /etc/hosts
EOF
  exit 2
fi

if (exec 3<>"/dev/tcp/127.0.0.1/$host_port") 2>/dev/null; then
  exec 3>&- 3<&- || true
  die "host port $host_port is already in use; stop the occupying service first (dev server: tmux kill-session -t rumahl-dev)"
fi
command -v ccache >/dev/null 2>&1 && have_ccache=1 || have_ccache=0
note "preflight ok (ccache: $([ "$have_ccache" = 1 ] && echo yes || echo no), jobs: $jobs)"

# ---- 1. Build -------------------------------------------------------------
if [ "$skip_build" = 1 ]; then
  [ -f "$output/images/bzImage" ] || die "--skip-build but $output/images/bzImage is missing"
  note "skipping build, reusing $output"
else
  build_args=(--output "$output" --artifacts "$state/artifacts" --jobs "$jobs")
  if [ "$have_ccache" = 1 ]; then
    build_args+=(--config-fragment "$image/ccache.fragment")
  fi
  note "building the image (first run: one to three hours; later runs use ccache)"
  "$image/build.sh" "${build_args[@]}"
fi

# ---- 2. Provision the dev seed -------------------------------------------
if [ "$fresh" = 1 ]; then
  rm -rf "$seed"
  rm -f "$data"
fi
if [ -f "$data" ] && [ -d "$seed" ]; then
  note "reusing seed $seed and data disk $data (--fresh to reprovision)"
else
  rm -rf "$seed"
  rm -f "$data"
  note "provisioning dev seed and data disk"
  "$repo/platform-buildroot/deployment/dev-provision.sh" \
    --seed "$seed" --data "$data" --host "$origin_host"
fi
password="$(awk '$1 == "alice" { $1=""; sub(/^ +/, ""); print }' "$seed/credentials.txt")"
[ -n "$password" ] || die "could not read the alice password from $seed/credentials.txt"

# ---- 3. Boot the VM -------------------------------------------------------
note "booting the VM (log: $log)"
rm -f "$pid_file"
boot_args=(--output "$output" --data "$data")
[ "$tpm" = 1 ] && boot_args+=(--tpm)
RUMAHL_BUILDROOT_OUTPUT="$output" "$image/boot.sh" "${boot_args[@]}" >"$log" 2>&1 &
vm_pid=$!
echo "$vm_pid" > "$pid_file"

shutdown_vm() {
  if [ -n "$vm_pid" ] && kill -0 "$vm_pid" 2>/dev/null; then
    note "stopping the VM"
    kill "$vm_pid" 2>/dev/null || true
    for _ in $(seq 1 50); do
      kill -0 "$vm_pid" 2>/dev/null || break
      sleep 0.2
    done
    kill -9 "$vm_pid" 2>/dev/null || true
  fi
}
trap shutdown_vm EXIT

note "waiting for https://$origin_host:$host_port (up to 4 minutes)"
ready=0
for _ in $(seq 1 240); do
  if kill -0 "$vm_pid" 2>/dev/null; then
    if (exec 3<>"/dev/tcp/127.0.0.1/$host_port") 2>/dev/null; then
      exec 3>&- 3<&- || true
      ready=1
      break
    fi
  else
    tail -40 "$log" >&2 || true
    die "the VM exited before the shell answered; see $log"
  fi
  sleep 1
done
[ "$ready" = 1 ] || { tail -40 "$log" >&2 || true; die "the shell did not answer on port $host_port; see $log"; }
note "the shell answered on https://$origin_host:$host_port"

# ---- 4. Acceptance checks -------------------------------------------------
note "running HTTPS acceptance checks"
checks=(
  --origin "https://$origin_host:$host_port"
  --ca "$seed/lib/rumahl-ca/ca.pem"
  --username alice
  --password "$password"
)
status=0
python3 "$repo/tests/e2e/hardware/check.py" "${checks[@]}" | tee "$state/acceptance.out" || status=$?

# ---- 5. Report ------------------------------------------------------------
if [ "$status" = 0 ]; then
  note "ACCEPTANCE PASSED — report: $state/acceptance.out"
else
  note "ACCEPTANCE FAILED (exit $status) — report: $state/acceptance.out, VM log: $log"
fi
if [ "$keep_vm" = 1 ]; then
  note "--keep-vm: the VM stays up at https://$origin_host:$host_port (stop with: kill \$(cat $pid_file))"
  vm_pid=""
else
  shutdown_vm
fi
exit "$status"
