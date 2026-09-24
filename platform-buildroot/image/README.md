# Buildroot browser milestone

This is a new, small `BR2_EXTERNAL` tree, informed by the prototype rather than
copied from it. The first target is QEMU x86-64. It uses the Buildroot-maintained
QEMU kernel configuration and hash files, systemd, Node with ICU and nginx.
ICU supplies the `Intl` API required by the shell renderer; Node without ICU
cannot load the SSR bundle. Docker, the
old product-specific services, desktop packages and development bridges are not
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

Place a certificate for `rumahl.home.arpa` and its private key in
`$RUMAHL_SEED/lib/rumahl-tls/fullchain.pem` and `key.pem`. Restrict the key to mode
`0600`. Use a locally trusted development CA and install only its public trust
certificate in the test browser. TLS verification stays enabled.

```sh
python3 platform-buildroot/image/create-data.py --seed "$RUMAHL_SEED" --output "$RUMAHL_DATA_IMAGE"
platform-buildroot/image/run-qemu.sh "$BR_OUTPUT" "$RUMAHL_DATA_IMAGE"
```

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
