# Buildroot browser milestone

## One-command acceptance

On a fresh Ubuntu 24.04 host, three commands take a clean clone to a fully
checked QEMU appliance:

```sh
# 1. host prerequisites (root for apt; also prints the hosts-entry line)
sudo platform-buildroot/image/host-setup.sh

# 2. once per host: resolve the QEMU origin locally
echo '127.0.0.1 rumahl.home.arpa' | sudo tee -a /etc/hosts

# 3. build, boot and check — unattended end to end
platform-buildroot/image/acceptance.sh
```

`acceptance.sh` runs the whole pipeline: preflight (tools, ≥ 20 GiB free
disk, hosts entry, free port 8443), image build with host ccache
(`ccache.fragment`, reuse makes rebuilds minutes instead of hours), dev seed
provisioning (alice/bob accounts, development CA, ext4 data disk), an
unattended QEMU boot, the HTTPS acceptance checks from
`tests/e2e/hardware/check.py`, and a clean VM shutdown. Everything reusable
lands under `buildroot/acceptance/` (Buildroot output, seed, data disk,
`acceptance.out` report, `qemu.log`). Useful flags: `--skip-build` after a
code change that only touches staged assets, `--fresh` to reprovision the
seed, `--tpm` to exercise the sealed secret store, `--keep-vm` to keep the
appliance up for manual poking afterwards.

This is a new, small `BR2_EXTERNAL` tree, informed by the prototype rather than
copied from it. The first target is QEMU x86-64. It uses the Buildroot-maintained
QEMU kernel configuration and hash files, systemd, Node with ICU and nginx.
ICU supplies the `Intl` API required by the shell renderer; Node without ICU
cannot load the SSR bundle. The image also ships the Docker engine, the runtime
supervisor and `tpm2-tools` so installed app containers and the TPM-sealed
secret store can run; local login does not depend on any of them. The old
product-specific services, desktop packages and development bridges are not
prerequisites for local login.

The system disk is read-only. An explicitly created second ext4 disk supplies
`/var`; the platform requires that mount before starting. Buildroot's default
volatile `/var` factory/overlay is disabled. Root login is disabled. TLS keys and
account databases are provisioned onto the data disk, never baked into the
system image. Do not use `-snapshot` on the data disk for persistence acceptance.

The current version is pinned in `buildroot-source.json`, including its archive
SHA-256. This is a development target, not a completed signed-update or hardware
release. A/B updates, boot verification, TPM provisioning, installer media and
additional boards need their own acceptance work.

## Build

`build.sh` automates the whole sequence below (fetch and verify Buildroot,
configure the external tree, build the toolchain, cross-compile the Rust
services, build the frontend, stage assets and build the image):

```sh
platform-buildroot/image/build.sh
```

It defaults to `<repo>/buildroot/{buildroot-<version>,output,artifacts}` and
accepts `--source`, `--output`, `--artifacts`, `--download`, `--jobs`,
`--defconfig`, `--trust-store`, `--first-boot-env` and `--config-fragment`
(repeatable; `image/ccache.fragment` enables the Buildroot host ccache, its
directory defaults to `~/.cache/buildroot`). The
manual steps, and the variables they export, are:

Use a Linux build host with the [Buildroot host prerequisites](https://buildroot.org/downloads/manual/manual.html#requirement-mandatory).
Read the pinned version and hash from `buildroot-source.json`; fetch and verify
the archive before extracting it. Paths below must be absolute, without spaces.
`RUMAHL_REPO` is this repository, `BR_SOURCE` is the extracted source tree,
`BR_OUTPUT` is a fresh output directory, and `RUMAHL_ARTIFACTS` is a **new** staging
path. The staging path may be configured before it exists.

```sh
make -C "$BR_SOURCE" O="$BR_OUTPUT" \
  BR2_EXTERNAL="$RUMAHL_REPO/platform-buildroot/image" rumahl_qemu_x86_64_defconfig
"$BR_SOURCE/utils/config" --file "$BR_OUTPUT/.config" \
  --set-str BR2_PACKAGE_RUMAHL_SHELL_ARTIFACTS "$RUMAHL_ARTIFACTS"
make -C "$BR_SOURCE" O="$BR_OUTPUT" olddefconfig
make -C "$BR_SOURCE" O="$BR_OUTPUT" toolchain
```

Build the platform with this image's compiler, not the host glibc. Keep its Cargo
output separate from native development binaries:

```sh
cd "$RUMAHL_REPO"
rustup target add x86_64-unknown-linux-gnu
CARGO_TARGET_DIR="$RUMAHL_REPO/target/buildroot" \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$BR_OUTPUT/host/bin/x86_64-buildroot-linux-gnu-gcc" \
CC_x86_64_unknown_linux_gnu="$BR_OUTPUT/host/bin/x86_64-buildroot-linux-gnu-gcc" \
  cargo build --locked --release --target x86_64-unknown-linux-gnu -p rumahl-platform-service
(cd frontend && pnpm install --frozen-lockfile && pnpm build)
python3 platform-buildroot/deployment/stage.py \
  --binary target/buildroot/x86_64-unknown-linux-gnu/release/rumahl-platform-service \
  --output "$RUMAHL_ARTIFACTS"
make -C "$BR_SOURCE" O="$BR_OUTPUT"
```

The stage command refuses to overwrite an existing directory and verifies the
renderer/client build match. Its SSR bundle runs without a package manager or
`node_modules` on the image. For an updated staging directory, explicitly run
`rumahl-shell-dirclean` before rebuilding the local Buildroot package.

The package installs two separately sandboxed service accounts and shared socket
groups. nginx's `www-data` user belongs only to `rumahl-web`, while only the
platform and renderer share `rumahl-ssr`. The platform has no Docker membership.
TLS is configured by the HTTPS edge; the renderer cannot access account SQLite.

## Real x86-64 hardware (UEFI)

`rumahl_x86_64_uefi_defconfig` builds a GPT disk image with an EFI system
partition, a read-only `rumahl-root` ext4 partition and an empty
`rumahl-data` partition. GRUB loads the kernel from the ESP; the data partition
is mounted at `/var` and provides `/home` and `/apps` bind mounts.

```sh
platform-buildroot/image/build.sh \
  --defconfig rumahl_x86_64_uefi_defconfig \
  --output <repo>/buildroot/uefi-output \
  --artifacts <repo>/buildroot/uefi-artifacts \
  --trust-store <package-trust-store.json> \
  --first-boot-env <first-boot.env>   # omit on a real device
```

Flash the whole disk image (this **erases** the target device):

```sh
sudo dd if=<repo>/buildroot/uefi-output/images/rumahl-disk.img \
  of=/dev/sdX bs=4M conv=fsync status=progress
```

For a QEMU/OVMF check before using hardware (add `--tpm` to start swtpm and
exercise the sealed secret-store key):

```sh
platform-buildroot/image/boot-uefi.sh --output <repo>/buildroot/uefi-output --tpm
```

On a real device the first boot runs an interactive console setup on
`/dev/console` (HDMI or serial) and asks for the admin account, display name,
hostname and password. It creates the device CA, an initial TLS certificate and
the first local account. If a TPM 2.0 device is present it seals a random
32-byte secret-store key and persists it at handle `0x81000001` (no PCR policy
yet). The host-config broker then adds the current IP SANs. If
`first-boot.env` was baked in, the setup runs unattended; `FIRST_BOOT_WIFI_*`
adds an iwd profile.

Avahi publishes `rumahl.local` over mDNS; clients need mDNS support. Wi-Fi is
managed by iwd (`iwctl`, and an optional first-boot profile).

The data partition is a fixed 8 GiB image. Grow it after flashing when the
device has more space (`parted`/`growpart` plus `resize2fs`).

Still open for this target: PCR/measured-boot binding of the TPM key,
Secure Boot signing, installer media, A/B updates and rollback, and physical
hardware acceptance.

## Provision the persistent disk

Create a private seed directory representing `/var`:

```sh
mkdir -p "$RUMAHL_SEED/lib/rumahl" "$RUMAHL_SEED/lib/rumahl-tls"
chmod 700 "$RUMAHL_SEED" "$RUMAHL_SEED/lib/rumahl" "$RUMAHL_SEED/lib/rumahl-tls"
```

Use a **native host build** of the platform provisioning CLI, as described in
[platform-service/README.md](../../../platform-service/README.md), with
`RUMAHL_STATE_DIR="$RUMAHL_SEED/lib/rumahl"`. Provision two local test users for
acceptance. Close the provisioning process before constructing the data image;
do not copy a database out of an actively running service.

Place a certificate for `rumahl.home.arpa` **and `*.apps.rumahl.home.arpa`** and its private key in
`$RUMAHL_SEED/lib/rumahl-tls/fullchain.pem` and `key.pem`. Restrict the key to mode
`0600`. Use a locally trusted development CA and install only its public trust
certificate in the test browser. TLS verification stays enabled.

```sh
python3 platform-buildroot/image/create-data.py --seed "$RUMAHL_SEED" --output "$RUMAHL_DATA_IMAGE"
platform-buildroot/image/run-qemu.sh "$BR_OUTPUT" "$RUMAHL_DATA_IMAGE"
```

`boot.sh` is a convenience wrapper that creates the data disk from `--seed`
when it is missing and starts QEMU:

```sh
platform-buildroot/image/boot.sh --output "$BR_OUTPUT" --seed "$RUMAHL_SEED"
```

Pass `--tpm` to `boot.sh` to start `swtpm` and attach a TPM 2.0 device. This
only supplies the device: creating the sealed platform key object that
`RUMAHL_SECRET_KEY_OBJECT` references remains device lifecycle work, and the
secret store fails closed until it is provisioned. Container app installation
needs the running Docker engine, the `rumahl-app-network` unit and the
`rumahl-runtime-supervisor` service; the platform service never talks to
Docker directly.

Dynamic app ports are allocated at runtime and persisted on the data disk.
To reach an installed app from the host in the development VM, forward the port
shown in its frame URL with `boot.sh --app-port N` (repeatable). Real hardware
needs no forwarding because the app listens on the device itself.

The data disk also carries the stable `/home` and `/apps` trees; the image binds
them to those paths at boot. The system state stays under `/var/lib/rumahl` and
is never mounted into the user- or app-visible path namespace. See
[filesystem layout](../../../docs/architecture/filesystem-layout.md) and
[storage and updates](../../../docs/architecture/storage-and-updates.md).

The data-image command uses `mke2fs -d` and exclusive file creation; it never
selects or formats a physical disk. QEMU exposes only guest HTTPS at host
`127.0.0.1:8443`. Resolve `rumahl.home.arpa` to `127.0.0.1` on the host and open
`https://rumahl.home.arpa:8443`. The QEMU board configures that exact public origin;
change both the origin and edge hostname when using a different deployment.

## Acceptance status

A full Buildroot compile and QEMU acceptance run were completed on a separate
Linux build host on 2026-09-24. The returned logs record 18 successful HTTPS
checks across four boots, including persisted sessions and selective logout.
See the [reviewed acceptance record](../../../docs/architecture/buildroot-acceptance.md)
for image hashes, source provenance and test limits. These are HTTP-client tests;
real-browser acceptance and renderer failure inside the VM remain open.
A green host-level test alone does not establish VM acceptance.

For VM acceptance: sign in independently as both users, restart the VM while
retaining the same data disk, verify both identities, then log out one user and
verify the old credential remains rejected after another restart. Stop the
renderer and verify `/recovery` and `/login` remain reachable. Record the
Buildroot version, source commit, shell build ID and image hashes, never tokens
or passwords.

Installed web apps also require local wildcard DNS for `*.apps.rumahl.home.arpa`
to resolve to the device (or the QEMU host when using port forwarding). A hosts
file cannot define wildcards; individual installation hostnames can be added for
a test. The same port and trusted certificate serve shell and isolated app hosts.
See [app hosting](../../docs/architecture/app-hosting.md) for publication and
explicit launch grants. This path still needs validation in a booted image.
